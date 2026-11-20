use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::{mpsc, RwLock};
use tracing::{info, warn};

use crate::error::CoreError;
use crate::identity::DeviceIdentity;
use crate::protocol::{
    read_frame, write_frame, AppMessage,
    MSG_TYPE_SESSION_APP, MSG_TYPE_SESSION_HANDSHAKE,
};
use crate::trust::{TrustedDevice, TrustStore};

const SESSION_NOISE_PATTERN: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";

pub struct EncryptedSession<S> {
    stream: S,
    transport: snow::TransportState,
    pub peer_device: TrustedDevice,
}

impl<S> EncryptedSession<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    pub async fn send_app_message(&mut self, msg: &AppMessage) -> Result<(), CoreError> {
        let json_bytes = serde_json::to_vec(msg)?;
        let mut enc_buf = vec![0u8; json_bytes.len() + 16];
        let enc_len = self.transport.write_message(&json_bytes, &mut enc_buf)
            .map_err(|e| CoreError::Noise(e.to_string()))?;
        write_frame(&mut self.stream, MSG_TYPE_SESSION_APP, &enc_buf[..enc_len]).await?;
        Ok(())
    }

    pub async fn read_app_message(&mut self) -> Result<AppMessage, CoreError> {
        let (msg_type, enc_payload) = read_frame(&mut self.stream).await?;
        if msg_type != MSG_TYPE_SESSION_APP {
            return Err(CoreError::Protocol(format!(
                "Unexpected message type in secure session: 0x{:02x}",
                msg_type
            )));
        }

        let mut dec_buf = vec![0u8; enc_payload.len()];
        let dec_len = self.transport.read_message(&enc_payload, &mut dec_buf)
            .map_err(|e| CoreError::Noise(e.to_string()))?;
        let msg: AppMessage = serde_json::from_slice(&dec_buf[..dec_len])?;
        Ok(msg)
    }
}

pub struct SessionManager {
    identity: DeviceIdentity,
    trust_store: Arc<TrustStore>,
    // Mapping from peer device_id to abort handle or shutdown trigger
    active_sessions: Arc<RwLock<HashMap<String, mpsc::Sender<()>>>>,
}

impl SessionManager {
    pub fn new(identity: DeviceIdentity, trust_store: Arc<TrustStore>) -> Self {
        Self {
            identity,
            trust_store,
            active_sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Server side taking full ownership of stream
    pub async fn accept_session_stream<S>(
        &self,
        mut stream: S,
    ) -> Result<(EncryptedSession<S>, mpsc::Receiver<()>), CoreError>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let params: snow::params::NoiseParams = SESSION_NOISE_PATTERN.parse()
            .map_err(|e| CoreError::Noise(format!("{:?}", e)))?;
        let builder = snow::Builder::new(params);
        let mut noise_responder = builder
            .local_private_key(&self.identity.noise_static_secret_bytes())
            .build_responder()
            .map_err(|e| CoreError::Noise(e.to_string()))?;

        let mut read_buf = [0u8; 1024];
        let mut write_buf = [0u8; 1024];

        // Step 1: Read client handshake msg 1 (-> e)
        let (msg_type1, client_msg1) = read_frame(&mut stream).await?;
        if msg_type1 != MSG_TYPE_SESSION_HANDSHAKE {
            return Err(CoreError::Protocol("Expected session handshake msg 1".to_string()));
        }
        noise_responder.read_message(&client_msg1, &mut read_buf)
            .map_err(|e| CoreError::Noise(format!("Handshake read 1 failed: {}", e)))?;

        // Step 2: Write responder handshake msg 2 (<- e, ee, s, es)
        let len2 = noise_responder.write_message(&[], &mut write_buf)
            .map_err(|e| CoreError::Noise(e.to_string()))?;
        write_frame(&mut stream, MSG_TYPE_SESSION_HANDSHAKE, &write_buf[..len2]).await?;

        // Step 3: Read client handshake msg 3 (-> s, se)
        let (msg_type3, client_msg3) = read_frame(&mut stream).await?;
        if msg_type3 != MSG_TYPE_SESSION_HANDSHAKE {
            return Err(CoreError::Protocol("Expected session handshake msg 3".to_string()));
        }
        noise_responder.read_message(&client_msg3, &mut read_buf)
            .map_err(|e| CoreError::Noise(format!("Handshake read 3 failed: {}", e)))?;

        // Verify remote static key against TrustStore
        let remote_static = noise_responder
            .get_remote_static()
            .ok_or_else(|| CoreError::Session("Missing remote static key in handshake".to_string()))?;
        let mut remote_key_arr = [0u8; 32];
        remote_key_arr.copy_from_slice(remote_static);

        let peer_device = match self.trust_store.get_by_noise_key(&remote_key_arr).await {
            Some(dev) => dev,
            None => {
                let untrusted_hex = hex::encode(remote_key_arr);
                warn!(
                    "SECURITY REJECTION: Dropping connection from untrusted peer (static key: {}). Dropped before application data.",
                    untrusted_hex
                );
                return Err(CoreError::UntrustedPeer(untrusted_hex));
            }
        };

        info!("Authenticated session accepted from trusted device: {} ({})", peer_device.device_name, peer_device.device_id);
        self.trust_store.update_last_seen(&peer_device.device_id).await;

        let transport = noise_responder.into_transport_mode()
            .map_err(|e| CoreError::Noise(e.to_string()))?;

        let (tx, rx) = mpsc::channel(1);
        self.active_sessions.write().await.insert(peer_device.device_id.clone(), tx);

        Ok((
            EncryptedSession {
                stream,
                transport,
                peer_device,
            },
            rx,
        ))
    }

    /// Client side: initiate secure session with peer
    pub async fn connect_to_peer<S>(
        &self,
        mut stream: S,
        expected_peer: &TrustedDevice,
    ) -> Result<EncryptedSession<S>, CoreError>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let expected_key_bytes = expected_peer.noise_static_key_bytes()?;

        let params: snow::params::NoiseParams = SESSION_NOISE_PATTERN.parse()
            .map_err(|e| CoreError::Noise(format!("{:?}", e)))?;
        let builder = snow::Builder::new(params);
        let mut noise_initiator = builder
            .local_private_key(&self.identity.noise_static_secret_bytes())
            .build_initiator()
            .map_err(|e| CoreError::Noise(e.to_string()))?;

        let mut read_buf = [0u8; 1024];
        let mut write_buf = [0u8; 1024];

        // Step 1: Write initiator handshake msg 1 (-> e)
        let len1 = noise_initiator.write_message(&[], &mut write_buf)
            .map_err(|e| CoreError::Noise(e.to_string()))?;
        write_frame(&mut stream, MSG_TYPE_SESSION_HANDSHAKE, &write_buf[..len1]).await?;

        // Step 2: Read responder handshake msg 2 (<- e, ee, s, es)
        let (msg_type2, resp_msg2) = read_frame(&mut stream).await?;
        if msg_type2 != MSG_TYPE_SESSION_HANDSHAKE {
            return Err(CoreError::Protocol("Expected session handshake msg 2".to_string()));
        }
        noise_initiator.read_message(&resp_msg2, &mut read_buf)
            .map_err(|e| CoreError::Noise(format!("Handshake read 2 failed: {}", e)))?;

        // Verify responder's static key matches expected peer key and is in TrustStore
        let remote_static = noise_initiator
            .get_remote_static()
            .ok_or_else(|| CoreError::Session("Missing remote static key in handshake".to_string()))?;
        if remote_static != expected_key_bytes {
            let actual_hex = hex::encode(remote_static);
            warn!("SECURITY ALERT: Remote peer static key {} did not match expected key {}!", actual_hex, expected_peer.noise_static_key_hex);
            return Err(CoreError::UntrustedPeer(actual_hex));
        }

        if !self.trust_store.is_noise_key_trusted(&expected_key_bytes).await {
            return Err(CoreError::UntrustedPeer(expected_peer.noise_static_key_hex.clone()));
        }

        // Step 3: Write initiator handshake msg 3 (-> s, se)
        let len3 = noise_initiator.write_message(&[], &mut write_buf)
            .map_err(|e| CoreError::Noise(e.to_string()))?;
        write_frame(&mut stream, MSG_TYPE_SESSION_HANDSHAKE, &write_buf[..len3]).await?;

        // Transition to transport mode
        let transport = noise_initiator.into_transport_mode()
            .map_err(|e| CoreError::Noise(e.to_string()))?;

        self.trust_store.update_last_seen(&expected_peer.device_id).await;

        let (tx, _rx) = mpsc::channel(1);
        self.active_sessions.write().await.insert(expected_peer.device_id.clone(), tx);

        Ok(EncryptedSession {
            stream,
            transport,
            peer_device: expected_peer.clone(),
        })
    }

    /// Close active session immediately (called on revoke)
    pub async fn close_session(&self, device_id: &str) {
        let mut map = self.active_sessions.write().await;
        if let Some(tx) = map.remove(device_id) {
            let _ = tx.send(()).await;
            info!("Session closed for revoked device: {}", device_id);
        }
    }

    /// Ping peer and measure round trip latency in milliseconds
    pub async fn measure_ping<S>(session: &mut EncryptedSession<S>) -> Result<u64, CoreError>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let start = Instant::now();
        let now_millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        session.send_app_message(&AppMessage::Ping { timestamp: now_millis }).await?;
        let resp = session.read_app_message().await?;
        match resp {
            AppMessage::Pong { timestamp: _ } => {
                let elapsed = start.elapsed().as_millis() as u64;
                Ok(elapsed)
            }
            other => Err(CoreError::Session(format!("Expected Pong, got {:?}", other))),
        }
    }
}
