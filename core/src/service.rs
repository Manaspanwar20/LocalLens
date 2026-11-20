use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::{broadcast, RwLock};
use tracing::{error, info, warn};

use crate::discovery::{DiscoveredDevice, DiscoveryManager};
use crate::error::CoreError;
use crate::identity::DeviceIdentity;
use crate::pairing::{PairingAuthMode, PairingManager, QrPairingPayload, SasConfirmation};
use crate::protocol::{read_frame, AppMessage, MSG_TYPE_PAIR_HANDSHAKE, MSG_TYPE_SESSION_HANDSHAKE};
use crate::secure_store::SecureStore;
use crate::session::SessionManager;
use crate::trust::{TrustedDevice, TrustStore};

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct IdentityDto {
    pub device_id: String,
    pub device_name: String,
    pub fingerprint: String,
    pub noise_public_key_hex: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum ServiceEvent {
    DiscoveredDevicesUpdated(Vec<DiscoveredDevice>),
    PairingSuccess {
        peer: TrustedDevice,
        sas: SasConfirmation,
    },
    PairingFailed(String),
    PairingModeExpired,
    SessionPingResult {
        device_id: String,
        latency_ms: u64,
    },
}

pub struct AppService {
    pub identity: Arc<RwLock<DeviceIdentity>>,
    pub secure_store: Arc<SecureStore>,
    pub trust_store: Arc<TrustStore>,
    pub discovery: Arc<DiscoveryManager>,
    pub pairing: Arc<PairingManager>,
    pub session: Arc<SessionManager>,
    pub local_port: RwLock<u16>,
    pub event_tx: broadcast::Sender<ServiceEvent>,
}

impl AppService {
    pub async fn init(custom_dir: Option<std::path::PathBuf>) -> Result<Arc<Self>, CoreError> {
        let secure_store = Arc::new(match custom_dir {
            Some(dir) => SecureStore::new(dir),
            None => SecureStore::default_store(),
        });

        let identity = DeviceIdentity::load_or_create(&secure_store, None)?;
        let identity_ref = Arc::new(RwLock::new(identity.clone()));

        let trust_store = Arc::new(TrustStore::new(secure_store.clone()));
        trust_store.load_or_init().await?;

        let discovery = Arc::new(DiscoveryManager::new()?);
        let pairing = Arc::new(PairingManager::new());
        let session = Arc::new(SessionManager::new(identity, trust_store.clone()));

        let (event_tx, _) = broadcast::channel(64);

        let service = Arc::new(Self {
            identity: identity_ref,
            secure_store,
            trust_store,
            discovery,
            pairing,
            session,
            local_port: RwLock::new(0),
            event_tx,
        });

        Ok(service)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ServiceEvent> {
        self.event_tx.subscribe()
    }

    pub async fn start_network_listener(self: &Arc<Self>, requested_port: u16) -> Result<u16, CoreError> {
        let addr = SocketAddr::from(([0, 0, 0, 0], requested_port));
        let listener = TcpListener::bind(addr).await?;
        let local_addr = listener.local_addr()?;
        let bound_port = local_addr.port();
        *self.local_port.write().await = bound_port;

        info!("LocalFirstFileSharing TCP listener active on 0.0.0.0:{}", bound_port);

        // Start mDNS advertisement
        let id = self.identity.read().await;
        self.discovery.start_advertising(
            &id.device_id,
            &id.device_name,
            bound_port,
            false,
            &id.fingerprint,
        ).await?;
        drop(id);

        let service = self.clone();
        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, peer_addr)) => {
                        let svc = service.clone();
                        tokio::spawn(async move {
                            svc.handle_incoming_connection(stream, peer_addr).await;
                        });
                    }
                    Err(e) => {
                        error!("TCP accept error: {}", e);
                    }
                }
            }
        });

        Ok(bound_port)
    }

    async fn handle_incoming_connection<S>(&self, mut stream: S, peer_addr: SocketAddr)
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
    {
        // Peek or read initial frame
        // In our protocol, the first frame determines whether it is a pairing attempt or session
        // Note: For pairing or session handshake, we read the first frame
        let frame_res = read_frame(&mut stream).await;
        let (msg_type, payload) = match frame_res {
            Ok(res) => res,
            Err(e) => {
                warn!("Connection from {} closed before valid frame: {}", peer_addr, e);
                return;
            }
        };

        match msg_type {
            MSG_TYPE_PAIR_HANDSHAKE => {
                info!("Incoming pairing connection from {}", peer_addr);
                // Wrap stream with pre-read frame
                let wrapped_stream = PrefixedStream::new(
                    stream,
                    prepend_frame_bytes(MSG_TYPE_PAIR_HANDSHAKE, &payload),
                );
                let mut pstream = wrapped_stream;
                let id = self.identity.read().await.clone();

                match self.pairing.handle_incoming_pairing(&mut pstream, &id, &self.trust_store).await {
                    Ok((peer, sas)) => {
                        info!("Pairing completed successfully with {}!", peer.device_name);
                        // Turn off pairing advertisement
                        let _ = self.discovery.update_pairing_mode(
                            &id.device_id,
                            &id.device_name,
                            *self.local_port.read().await,
                            false,
                            &id.fingerprint,
                        ).await;

                        let _ = self.event_tx.send(ServiceEvent::PairingSuccess {
                            peer,
                            sas,
                        });
                    }
                    Err(e) => {
                        warn!("Pairing failed for {}: {}", peer_addr, e);
                        let _ = self.event_tx.send(ServiceEvent::PairingFailed(e.to_string()));
                    }
                }
            }
            MSG_TYPE_SESSION_HANDSHAKE => {
                info!("Incoming session handshake from {}", peer_addr);
                let wrapped_stream = PrefixedStream::new(
                    stream,
                    prepend_frame_bytes(MSG_TYPE_SESSION_HANDSHAKE, &payload),
                );

                match self.session.accept_session_stream(wrapped_stream).await {
                    Ok((mut secure_session, mut shutdown_rx)) => {
                        info!("Encrypted session established with {}", secure_session.peer_device.device_name);

                        // Loop handling session messages (Ping, Pong, etc.)
                        loop {
                            tokio::select! {
                                _ = shutdown_rx.recv() => {
                                    info!("Session terminated for {}", secure_session.peer_device.device_name);
                                    break;
                                }
                                app_msg = secure_session.read_app_message() => {
                                    match app_msg {
                                        Ok(AppMessage::Ping { timestamp }) => {
                                            if let Err(e) = secure_session.send_app_message(&AppMessage::Pong { timestamp }).await {
                                                warn!("Failed to send pong: {}", e);
                                                break;
                                            }
                                        }
                                        Ok(AppMessage::Disconnect { reason }) => {
                                            info!("Peer disconnected: {}", reason);
                                            break;
                                        }
                                        Ok(other) => {
                                            info!("Received session message: {:?}", other);
                                        }
                                        Err(e) => {
                                            warn!("Session read error or EOF from {}: {}", secure_session.peer_device.device_name, e);
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Secure session rejected from {}: {}", peer_addr, e);
                    }
                }
            }
            other => {
                warn!(
                    "SECURITY DROP: Unknown frame type 0x{:02x} from {}. Dropped immediately.",
                    other, peer_addr
                );
            }
        }
    }

    pub async fn get_identity(&self) -> IdentityDto {
        let id = self.identity.read().await;
        IdentityDto {
            device_id: id.device_id.clone(),
            device_name: id.device_name.clone(),
            fingerprint: id.fingerprint.clone(),
            noise_public_key_hex: hex::encode(id.noise_public_key_bytes()),
        }
    }

    pub async fn update_device_name(&self, new_name: String) -> Result<(), CoreError> {
        {
            let mut id = self.identity.write().await;
            id.set_device_name(new_name.clone(), &self.secure_store)?;
        }
        let id = self.identity.read().await;
        let port = *self.local_port.read().await;
        if port > 0 {
            self.discovery.update_pairing_mode(
                &id.device_id,
                &new_name,
                port,
                self.pairing.is_in_pairing_mode().await,
                &id.fingerprint,
            ).await?;
        }
        Ok(())
    }

    pub async fn start_pairing_mode(&self) -> Result<QrPairingPayload, CoreError> {
        let (secret_hex, code, _expires_in) = self.pairing.create_session().await?;
        let id = self.identity.read().await;
        let port = *self.local_port.read().await;

        // Update mDNS to advertise pairing_mode=1
        self.discovery.update_pairing_mode(
            &id.device_id,
            &id.device_name,
            port,
            true,
            &id.fingerprint,
        ).await?;

        // Resolve local IP addresses
        let mut addrs = Vec::new();
        if let Ok(if_addrs) = if_addrs::get_if_addrs() {
            for iface in if_addrs {
                if !iface.is_loopback() {
                    let ip = iface.ip();
                    if let std::net::IpAddr::V4(v4) = ip {
                        if !v4.is_link_local() {
                            addrs.push(v4.to_string());
                        }
                    }
                }
            }
        }
        if addrs.is_empty() {
            addrs.push("127.0.0.1".to_string());
        }

        let payload = QrPairingPayload {
            version: 1,
            device_id: id.device_id.clone(),
            device_name: id.device_name.clone(),
            fingerprint: id.fingerprint.clone(),
            addrs,
            port,
            secret: secret_hex,
            code,
        };

        // Spawn timer to auto-disable pairing mode after timeout
        let discovery_clone = self.discovery.clone();
        let pairing_clone = self.pairing.clone();
        let id_clone = id.clone();
        let event_tx_clone = self.event_tx.clone();

        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_secs(crate::pairing::PAIRING_EXPIRY_SECS)).await;
            if pairing_clone.is_in_pairing_mode().await {
                pairing_clone.cancel_session().await;
                let _ = discovery_clone.update_pairing_mode(
                    &id_clone.device_id,
                    &id_clone.device_name,
                    port,
                    false,
                    &id_clone.fingerprint,
                ).await;
                let _ = event_tx_clone.send(ServiceEvent::PairingModeExpired);
            }
        });

        Ok(payload)
    }

    pub async fn cancel_pairing_mode(&self) -> Result<(), CoreError> {
        self.pairing.cancel_session().await;
        let id = self.identity.read().await;
        let port = *self.local_port.read().await;
        if port > 0 {
            self.discovery.update_pairing_mode(
                &id.device_id,
                &id.device_name,
                port,
                false,
                &id.fingerprint,
            ).await?;
        }
        Ok(())
    }

    pub async fn initiate_pairing_to_peer(
        &self,
        peer_addr: &str,
        mode: PairingAuthMode,
        secret_or_code_str: &str,
    ) -> Result<(TrustedDevice, SasConfirmation), CoreError> {
        let mut stream = tokio::net::TcpStream::connect(peer_addr).await?;
        let secret_or_code_bytes = match mode {
            PairingAuthMode::QrSecret => hex::decode(secret_or_code_str)
                .map_err(|e| CoreError::Pairing(format!("Invalid hex pairing secret: {}", e)))?,
            PairingAuthMode::NumericCode => secret_or_code_str.as_bytes().to_vec(),
        };

        let id = self.identity.read().await.clone();
        let (trusted, sas) = PairingManager::initiate_pairing(
            &mut stream,
            &id,
            mode,
            &secret_or_code_bytes,
            &self.trust_store,
        ).await?;

        let _ = self.event_tx.send(ServiceEvent::PairingSuccess {
            peer: trusted.clone(),
            sas: sas.clone(),
        });

        Ok((trusted, sas))
    }

    pub async fn connect_and_ping(&self, peer_addr: &str, device_id: &str) -> Result<u64, CoreError> {
        let expected_peer = self.trust_store.get_by_device_id(device_id).await
            .ok_or_else(|| CoreError::DeviceNotFound(device_id.to_string()))?;

        let stream = tokio::net::TcpStream::connect(peer_addr).await?;
        let mut secure_session = self.session.connect_to_peer(stream, &expected_peer).await?;

        let latency_ms = SessionManager::measure_ping(&mut secure_session).await?;

        let _ = self.event_tx.send(ServiceEvent::SessionPingResult {
            device_id: device_id.to_string(),
            latency_ms,
        });

        Ok(latency_ms)
    }

    pub async fn start_discovery_scan(&self) -> Result<(), CoreError> {
        let id = self.identity.read().await.device_id.clone();
        self.discovery.start_scan(self.trust_store.clone(), id).await
    }

    pub async fn list_trusted_devices(&self) -> Vec<TrustedDevice> {
        self.trust_store.list_devices().await
    }

    pub async fn rename_trusted_device(&self, device_id: &str, new_name: &str) -> Result<TrustedDevice, CoreError> {
        self.trust_store.rename_device(device_id, new_name).await
    }

    pub async fn revoke_trusted_device(&self, device_id: &str) -> Result<TrustedDevice, CoreError> {
        // Revoke from store
        let revoked = self.trust_store.revoke_device(device_id).await?;
        // Immediately drop any live sessions
        self.session.close_session(device_id).await;
        Ok(revoked)
    }
}

fn prepend_frame_bytes(msg_type: u8, payload: &[u8]) -> Vec<u8> {
    let mut vec = Vec::with_capacity(8 + payload.len());
    vec.extend_from_slice(crate::protocol::PROTOCOL_MAGIC);
    vec.push(crate::protocol::PROTOCOL_VERSION);
    vec.push(msg_type);
    vec.extend_from_slice(&(payload.len() as u16).to_be_bytes());
    vec.extend_from_slice(payload);
    vec
}

/// Helper struct for prepending read bytes back into an async stream
pub struct PrefixedStream<S> {
    stream: S,
    prefix: Vec<u8>,
    prefix_pos: usize,
}

impl<S> PrefixedStream<S> {
    pub fn new(stream: S, prefix: Vec<u8>) -> Self {
        Self {
            stream,
            prefix,
            prefix_pos: 0,
        }
    }
}

impl<S> tokio::io::AsyncRead for PrefixedStream<S>
where
    S: tokio::io::AsyncRead + Unpin,
{
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        if self.prefix_pos < self.prefix.len() {
            let available = self.prefix.len() - self.prefix_pos;
            let to_copy = available.min(buf.remaining());
            buf.put_slice(&self.prefix[self.prefix_pos..self.prefix_pos + to_copy]);
            self.prefix_pos += to_copy;
            return std::task::Poll::Ready(Ok(()));
        }

        std::pin::Pin::new(&mut self.stream).poll_read(cx, buf)
    }
}

impl<S> tokio::io::AsyncWrite for PrefixedStream<S>
where
    S: tokio::io::AsyncWrite + Unpin,
{
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        std::pin::Pin::new(&mut self.stream).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        std::pin::Pin::new(&mut self.stream).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        std::pin::Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}
