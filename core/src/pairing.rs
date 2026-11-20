use std::time::{Duration, Instant};
use hkdf::Hkdf;
use rand::rngs::OsRng;
use rand::Rng;
use sha2::Sha256;
use tokio::sync::Mutex;
use serde::{Deserialize, Serialize};

use crate::error::CoreError;
use crate::identity::DeviceIdentity;
use crate::protocol::{
    read_frame, write_frame, PairingExchangeInfo,
    MSG_TYPE_PAIR_HANDSHAKE, MSG_TYPE_PAIR_INFO,
};
use crate::trust::{TrustedDevice, TrustStore};

pub const PAIRING_EXPIRY_SECS: u64 = 120; // 2 minutes
pub const MAX_FAILED_ATTEMPTS: u32 = 5;
pub const LOCKOUT_DURATION_SECS: u64 = 60; // 60s lockout after 5 failures

const NOISE_PATTERN: &str = "Noise_XXpsk3_25519_ChaChaPoly_BLAKE2s";
const HKDF_SALT_QR: &[u8] = b"localshare-pairing-qr-v1";
const HKDF_SALT_CODE: &[u8] = b"localshare-pairing-code-v1";
const HKDF_INFO: &[u8] = b"pairing-psk-32";

const SAS_EMOJI_TABLE: [&str; 64] = [
    "🦊", "🐼", "🦁", "🐨", "🐯", "🐸", "🐙", "🦄",
    "🦅", "🦉", "🦋", "🐬", "🐢", "🐳", "🍀", "🌸",
    "🌻", "🍎", "🍓", "🍒", "🍕", "🍩", "🚀", "🛸",
    "🚗", "🚲", "⛵", "✈️", "⭐", "🌙", "☀️", "⚡",
    "🔥", "💎", "🛡️", "🔑", "🔔", "🎯", "🎨", "🎸",
    "🏆", "⚽", "🏀", "🎮", "🕹️", "🧩", "📦", "💡",
    "📚", "🔮", "🔭", "🧭", "⚓", "🪐", "🌈", "🌋",
    "🏔️", "🏝️", "🌊", "🍄", "🥑", "🍉", "🎈", "🎁",
];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QrPairingPayload {
    pub version: u8,
    pub device_id: String,
    pub device_name: String,
    pub fingerprint: String,
    pub addrs: Vec<String>,
    pub port: u16,
    pub secret: String, // hex encoded 32 bytes
    pub code: String,   // 6-digit numeric fallback
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SasConfirmation {
    pub numeric: String,
    pub emoji: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PairingAuthMode {
    QrSecret,
    NumericCode,
}

pub struct ActivePairingSession {
    pub pairing_secret: [u8; 32],
    pub numeric_code: String,
    pub created_at: Instant,
    pub expires_at: Instant,
    pub consumed: bool,
}

pub struct PairingManager {
    current_session: Mutex<Option<ActivePairingSession>>,
    failed_attempts: Mutex<u32>,
    locked_until: Mutex<Option<Instant>>,
}

impl PairingManager {
    pub fn new() -> Self {
        Self {
            current_session: Mutex::new(None),
            failed_attempts: Mutex::new(0),
            locked_until: Mutex::new(None),
        }
    }

    pub async fn create_session(&self) -> Result<(String, String, u64), CoreError> {
        let now = Instant::now();
        // Check rate-limiting lockout
        let mut lock = self.locked_until.lock().await;
        if let Some(until) = *lock {
            if now < until {
                let remaining = until.duration_since(now).as_secs() + 1;
                return Err(CoreError::PairingRateLimited(remaining));
            } else {
                *lock = None;
                *self.failed_attempts.lock().await = 0;
            }
        }

        // Generate 32 bytes of cryptographic randomness for QR
        let mut secret = [0u8; 32];
        use rand::RngCore;
        OsRng.fill_bytes(&mut secret);

        // Generate 6-digit numeric fallback code
        let num: u32 = rand::thread_rng().gen_range(100_000..1_000_000);
        let code = num.to_string();

        let expires_at = now + Duration::from_secs(PAIRING_EXPIRY_SECS);

        let mut session_lock = self.current_session.lock().await;
        *session_lock = Some(ActivePairingSession {
            pairing_secret: secret,
            numeric_code: code.clone(),
            created_at: now,
            expires_at,
            consumed: false,
        });

        let secret_hex = hex::encode(secret);
        Ok((secret_hex, code, PAIRING_EXPIRY_SECS))
    }

    pub async fn cancel_session(&self) {
        let mut session_lock = self.current_session.lock().await;
        *session_lock = None;
    }

    pub async fn is_in_pairing_mode(&self) -> bool {
        let session_lock = self.current_session.lock().await;
        if let Some(session) = &*session_lock {
            !session.consumed && Instant::now() < session.expires_at
        } else {
            false
        }
    }

    pub async fn record_failure(&self) {
        let mut fails = self.failed_attempts.lock().await;
        *fails += 1;
        if *fails >= MAX_FAILED_ATTEMPTS {
            let mut lock = self.locked_until.lock().await;
            *lock = Some(Instant::now() + Duration::from_secs(LOCKOUT_DURATION_SECS));
            tracing::warn!(
                "Pairing rate limit triggered! Locked out for {} seconds",
                LOCKOUT_DURATION_SECS
            );
        }
    }

    pub async fn record_success(&self) {
        let mut fails = self.failed_attempts.lock().await;
        *fails = 0;
        let mut lock = self.locked_until.lock().await;
        *lock = None;
    }

    pub fn derive_psk(mode: &PairingAuthMode, secret_or_code: &[u8]) -> [u8; 32] {
        let salt = match mode {
            PairingAuthMode::QrSecret => HKDF_SALT_QR,
            PairingAuthMode::NumericCode => HKDF_SALT_CODE,
        };
        let hk = Hkdf::<Sha256>::new(Some(salt), secret_or_code);
        let mut psk = [0u8; 32];
        hk.expand(HKDF_INFO, &mut psk)
            .expect("HKDF expand 32 bytes for PSK");
        psk
    }

    pub fn derive_sas(handshake_hash: &[u8]) -> SasConfirmation {
        // Derive 6 digits
        let mut num_bytes = [0u8; 4];
        num_bytes.copy_from_slice(&handshake_hash[0..4]);
        let raw_num = u32::from_be_bytes(num_bytes);
        let digits = format!("{:06}", (raw_num % 900_000) + 100_000);
        let formatted_numeric = format!("{} {}", &digits[0..3], &digits[3..6]);

        // Derive 4 emoji
        let mut emoji = Vec::with_capacity(4);
        for i in 4..8 {
            let idx = (handshake_hash[i] as usize) % SAS_EMOJI_TABLE.len();
            emoji.push(SAS_EMOJI_TABLE[idx].to_string());
        }

        SasConfirmation {
            numeric: formatted_numeric,
            emoji,
        }
    }

    /// Server side of pairing handshake
    pub async fn handle_incoming_pairing<S>(
        &self,
        stream: &mut S,
        identity: &DeviceIdentity,
        trust_store: &TrustStore,
    ) -> Result<(TrustedDevice, SasConfirmation), CoreError>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        // 1. Verify pairing session validity
        let now = Instant::now();
        {
            let lock = self.locked_until.lock().await;
            if let Some(until) = *lock {
                if now < until {
                    let rem = until.duration_since(now).as_secs() + 1;
                    return Err(CoreError::PairingRateLimited(rem));
                }
            }
        }

        let (pairing_secret, numeric_code) = {
            let session_lock = self.current_session.lock().await;
            if let Some(session) = &*session_lock {
                if session.consumed {
                    return Err(CoreError::PairingSecretInvalid);
                }
                if now >= session.expires_at {
                    return Err(CoreError::PairingExpired);
                }
                (session.pairing_secret, session.numeric_code.clone())
            } else {
                return Err(CoreError::PairingSecretInvalid);
            }
        };

        // Handshake Step 1: Read client message 1 (1 byte mode flag + Noise message 1)
        let (msg_type, client_msg1) = match read_frame(stream).await {
            Ok(res) => res,
            Err(e) => {
                self.record_failure().await;
                return Err(e);
            }
        };

        if msg_type != MSG_TYPE_PAIR_HANDSHAKE || client_msg1.is_empty() {
            self.record_failure().await;
            return Err(CoreError::Protocol("Expected pairing handshake frame".to_string()));
        }

        let mode_byte = client_msg1[0];
        let noise_msg1 = &client_msg1[1..];

        let (_mode, psk) = match mode_byte {
            0x01 => {
                let psk = Self::derive_psk(&PairingAuthMode::QrSecret, &pairing_secret);
                (PairingAuthMode::QrSecret, psk)
            }
            0x02 => {
                let psk = Self::derive_psk(&PairingAuthMode::NumericCode, numeric_code.as_bytes());
                (PairingAuthMode::NumericCode, psk)
            }
            _ => {
                self.record_failure().await;
                return Err(CoreError::Protocol("Invalid pairing auth mode".to_string()));
            }
        };

        // Initialize Snow responder with Noise_XXpsk3
        let params: snow::params::NoiseParams = NOISE_PATTERN.parse()
            .map_err(|e| CoreError::Noise(format!("{:?}", e)))?;
        let builder = snow::Builder::new(params);
        let mut noise_responder = builder
            .local_private_key(&identity.noise_static_secret_bytes())
            .psk(3, &psk)
            .build_responder()
            .map_err(|e| CoreError::Noise(e.to_string()))?;

        let mut read_buf = [0u8; 1024];
        let mut write_buf = [0u8; 1024];

        // Process Client Msg 1 (-> e)
        if let Err(e) = noise_responder.read_message(noise_msg1, &mut read_buf) {
            self.record_failure().await;
            return Err(CoreError::Noise(format!("Failed to read handshake msg 1: {}", e)));
        }

        // Handshake Step 2: Responder sends Msg 2 (<- e, ee, s, es)
        let len2 = match noise_responder.write_message(&[], &mut write_buf) {
            Ok(l) => l,
            Err(e) => {
                self.record_failure().await;
                return Err(CoreError::Noise(e.to_string()));
            }
        };
        write_frame(stream, MSG_TYPE_PAIR_HANDSHAKE, &write_buf[..len2]).await?;

        // Handshake Step 3: Responder reads Client Msg 3 (-> s, se, psk)
        let (msg_type3, client_msg3) = match read_frame(stream).await {
            Ok(res) => res,
            Err(e) => {
                self.record_failure().await;
                return Err(e);
            }
        };

        if msg_type3 != MSG_TYPE_PAIR_HANDSHAKE {
            self.record_failure().await;
            return Err(CoreError::Protocol("Expected pairing handshake msg 3".to_string()));
        }

        if let Err(e) = noise_responder.read_message(&client_msg3, &mut read_buf) {
            self.record_failure().await;
            tracing::warn!("Pairing authentication failed: invalid secret/code ({})", e);
            return Err(CoreError::Pairing("Authentication failed: invalid pairing secret or code".to_string()));
        }

        // Handshake completed! Extract peer's static Noise public key
        let peer_noise_static = noise_responder
            .get_remote_static()
            .ok_or_else(|| CoreError::Pairing("Remote static key missing".to_string()))?;
        let mut peer_noise_static_arr = [0u8; 32];
        peer_noise_static_arr.copy_from_slice(peer_noise_static);

        // Compute Handshake Hash & SAS
        let handshake_hash = noise_responder.get_handshake_hash();
        let sas = Self::derive_sas(handshake_hash);

        // Transition to transport state
        let mut transport = noise_responder.into_transport_mode()
            .map_err(|e| CoreError::Noise(e.to_string()))?;

        // Exchange Pairing Info over encrypted channel
        // Send our info
        let our_info = PairingExchangeInfo {
            device_id: identity.device_id.clone(),
            device_name: identity.device_name.clone(),
            public_key_hex: hex::encode(identity.verifying_key_bytes()),
            noise_static_key_hex: hex::encode(identity.noise_public_key_bytes()),
            fingerprint: identity.fingerprint.clone(),
        };
        let our_info_bytes = serde_json::to_vec(&our_info)?;
        let mut enc_buf = vec![0u8; our_info_bytes.len() + 16];
        let enc_len = transport.write_message(&our_info_bytes, &mut enc_buf)
            .map_err(|e| CoreError::Noise(e.to_string()))?;
        write_frame(stream, MSG_TYPE_PAIR_INFO, &enc_buf[..enc_len]).await?;

        // Read peer's info
        let (info_type, peer_enc_info) = read_frame(stream).await?;
        if info_type != MSG_TYPE_PAIR_INFO {
            return Err(CoreError::Protocol("Expected pairing info frame".to_string()));
        }
        let mut dec_buf = vec![0u8; peer_enc_info.len()];
        let dec_len = transport.read_message(&peer_enc_info, &mut dec_buf)
            .map_err(|e| CoreError::Noise(e.to_string()))?;
        let peer_info: PairingExchangeInfo = serde_json::from_slice(&dec_buf[..dec_len])?;

        // Verify that peer's advertised noise key matches authenticated handshake static key
        let advertised_noise_bytes = hex::decode(&peer_info.noise_static_key_hex)
            .map_err(|e| CoreError::Pairing(format!("Invalid peer noise key hex: {}", e)))?;
        if advertised_noise_bytes != peer_noise_static_arr {
            return Err(CoreError::Pairing("Peer noise static key mismatch".to_string()));
        }

        let mut peer_pubkey_arr = [0u8; 32];
        let pubkey_bytes = hex::decode(&peer_info.public_key_hex)
            .map_err(|e| CoreError::Pairing(format!("Invalid peer pubkey hex: {}", e)))?;
        if pubkey_bytes.len() != 32 {
            return Err(CoreError::Pairing("Peer public key length must be 32 bytes".to_string()));
        }
        peer_pubkey_arr.copy_from_slice(&pubkey_bytes);

        // Store peer in trust store
        let trusted = TrustedDevice::new(
            peer_info.device_id,
            peer_info.device_name,
            &peer_pubkey_arr,
            &peer_noise_static_arr,
            peer_info.fingerprint,
        );
        trust_store.add_device(trusted.clone()).await?;

        // Mark single-use pairing session as consumed and reset failure counters
        {
            let mut session_lock = self.current_session.lock().await;
            if let Some(session) = &mut *session_lock {
                session.consumed = true;
            }
        }
        self.record_success().await;

        tracing::info!("Pairing succeeded with peer {} ({})", trusted.device_name, trusted.device_id);
        Ok((trusted, sas))
    }

    /// Client side of pairing handshake (Initiator, e.g. connecting device)
    pub async fn initiate_pairing<S>(
        stream: &mut S,
        identity: &DeviceIdentity,
        mode: PairingAuthMode,
        pairing_secret_or_code: &[u8],
        trust_store: &TrustStore,
    ) -> Result<(TrustedDevice, SasConfirmation), CoreError>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        let psk = Self::derive_psk(&mode, pairing_secret_or_code);

        let params: snow::params::NoiseParams = NOISE_PATTERN.parse()
            .map_err(|e| CoreError::Noise(format!("{:?}", e)))?;
        let builder = snow::Builder::new(params);
        let mut noise_initiator = builder
            .local_private_key(&identity.noise_static_secret_bytes())
            .psk(3, &psk)
            .build_initiator()
            .map_err(|e| CoreError::Noise(e.to_string()))?;

        let mut read_buf = [0u8; 1024];
        let mut write_buf = [0u8; 1024];

        // Step 1: Write Msg 1 (-> e), prepending 1-byte mode flag
        let mode_byte: u8 = match mode {
            PairingAuthMode::QrSecret => 0x01,
            PairingAuthMode::NumericCode => 0x02,
        };
        let len1 = noise_initiator.write_message(&[], &mut write_buf[1..])
            .map_err(|e| CoreError::Noise(e.to_string()))?;
        write_buf[0] = mode_byte;
        write_frame(stream, MSG_TYPE_PAIR_HANDSHAKE, &write_buf[..len1 + 1]).await?;

        // Step 2: Read Msg 2 (<- e, ee, s, es)
        let (msg_type2, resp_msg2) = read_frame(stream).await?;
        if msg_type2 != MSG_TYPE_PAIR_HANDSHAKE {
            return Err(CoreError::Protocol("Expected pairing handshake msg 2".to_string()));
        }
        noise_initiator.read_message(&resp_msg2, &mut read_buf)
            .map_err(|e| CoreError::Noise(format!("Failed to read msg 2: {}", e)))?;

        // Step 3: Write Msg 3 (-> s, se, psk)
        let len3 = noise_initiator.write_message(&[], &mut write_buf)
            .map_err(|e| CoreError::Noise(format!("Failed to write msg 3: {}", e)))?;
        write_frame(stream, MSG_TYPE_PAIR_HANDSHAKE, &write_buf[..len3]).await?;

        // Handshake completed!
        let peer_noise_static = noise_initiator
            .get_remote_static()
            .ok_or_else(|| CoreError::Pairing("Remote static key missing".to_string()))?;
        let mut peer_noise_static_arr = [0u8; 32];
        peer_noise_static_arr.copy_from_slice(peer_noise_static);

        let handshake_hash = noise_initiator.get_handshake_hash();
        let sas = Self::derive_sas(handshake_hash);

        let mut transport = noise_initiator.into_transport_mode()
            .map_err(|e| CoreError::Noise(e.to_string()))?;

        // Read server info first
        let (info_type, server_enc_info) = read_frame(stream).await?;
        if info_type != MSG_TYPE_PAIR_INFO {
            return Err(CoreError::Protocol("Expected server pairing info frame".to_string()));
        }
        let mut dec_buf = vec![0u8; server_enc_info.len()];
        let dec_len = transport.read_message(&server_enc_info, &mut dec_buf)
            .map_err(|e| CoreError::Noise(e.to_string()))?;
        let server_info: PairingExchangeInfo = serde_json::from_slice(&dec_buf[..dec_len])?;

        // Send our client info
        let our_info = PairingExchangeInfo {
            device_id: identity.device_id.clone(),
            device_name: identity.device_name.clone(),
            public_key_hex: hex::encode(identity.verifying_key_bytes()),
            noise_static_key_hex: hex::encode(identity.noise_public_key_bytes()),
            fingerprint: identity.fingerprint.clone(),
        };
        let our_info_bytes = serde_json::to_vec(&our_info)?;
        let mut enc_buf = vec![0u8; our_info_bytes.len() + 16];
        let enc_len = transport.write_message(&our_info_bytes, &mut enc_buf)
            .map_err(|e| CoreError::Noise(e.to_string()))?;
        write_frame(stream, MSG_TYPE_PAIR_INFO, &enc_buf[..enc_len]).await?;

        // Verify noise key match
        let advertised_noise_bytes = hex::decode(&server_info.noise_static_key_hex)
            .map_err(|e| CoreError::Pairing(format!("Invalid server noise key hex: {}", e)))?;
        if advertised_noise_bytes != peer_noise_static_arr {
            return Err(CoreError::Pairing("Server noise static key mismatch".to_string()));
        }

        let mut server_pubkey_arr = [0u8; 32];
        let pubkey_bytes = hex::decode(&server_info.public_key_hex)
            .map_err(|e| CoreError::Pairing(format!("Invalid server pubkey hex: {}", e)))?;
        if pubkey_bytes.len() != 32 {
            return Err(CoreError::Pairing("Server public key length must be 32 bytes".to_string()));
        }
        server_pubkey_arr.copy_from_slice(&pubkey_bytes);

        let trusted = TrustedDevice::new(
            server_info.device_id,
            server_info.device_name,
            &server_pubkey_arr,
            &peer_noise_static_arr,
            server_info.fingerprint,
        );
        trust_store.add_device(trusted.clone()).await?;

        Ok((trusted, sas))
    }
}
