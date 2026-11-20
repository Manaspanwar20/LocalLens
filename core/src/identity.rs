use ed25519_dalek::{SigningKey, VerifyingKey};
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret as X25519StaticSecret};
use sha2::{Digest, Sha256};
use hkdf::Hkdf;
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;
use uuid::Uuid;

use crate::error::CoreError;
use crate::secure_store::SecureStore;

const IDENTITY_FILENAME: &str = "identity.enc";
const HKDF_INFO_X25519: &[u8] = b"localshare-identity-v1-x25519";
const HKDF_SALT: &[u8] = b"localshare-identity-salt-v1";

#[derive(Serialize, Deserialize)]
struct StoredIdentity {
    pub device_id: String,
    pub device_name: String,
    pub master_seed: Vec<u8>,
}

#[derive(Clone)]
pub struct DeviceIdentity {
    pub device_id: String,
    pub device_name: String,
    signing_key: SigningKey,
    verifying_key: VerifyingKey,
    noise_static_secret: [u8; 32],
    noise_public_key: [u8; 32],
    pub fingerprint: String,
}

impl DeviceIdentity {
    pub fn new(device_name: Option<String>) -> Self {
        let mut master_seed = [0u8; 32];
        use rand::RngCore;
        OsRng.fill_bytes(&mut master_seed);

        let device_id = Uuid::new_v4().to_string();
        let name = device_name.unwrap_or_else(|| {
            std::env::var("COMPUTERNAME")
                .or_else(|_| std::env::var("HOSTNAME"))
                .unwrap_or_else(|_| "LocalShare Device".to_string())
        });

        let identity = Self::from_seed(&master_seed, device_id, name);
        master_seed.zeroize();
        identity
    }

    pub fn from_seed(master_seed: &[u8; 32], device_id: String, device_name: String) -> Self {
        let signing_key = SigningKey::from_bytes(master_seed);
        let verifying_key = signing_key.verifying_key();

        // Derive X25519 secret for Noise handshakes via HKDF
        let hk = Hkdf::<Sha256>::new(Some(HKDF_SALT), master_seed);
        let mut x25519_secret_bytes = [0u8; 32];
        hk.expand(HKDF_INFO_X25519, &mut x25519_secret_bytes)
            .expect("HKDF expand 32 bytes");

        let static_secret = X25519StaticSecret::from(x25519_secret_bytes);
        let public_key = X25519PublicKey::from(&static_secret);

        let mut hasher = Sha256::new();
        hasher.update(verifying_key.as_bytes());
        let fingerprint = hex::encode(hasher.finalize());

        Self {
            device_id,
            device_name,
            signing_key,
            verifying_key,
            noise_static_secret: x25519_secret_bytes,
            noise_public_key: *public_key.as_bytes(),
            fingerprint,
        }
    }

    pub fn verifying_key_bytes(&self) -> [u8; 32] {
        *self.verifying_key.as_bytes()
    }

    pub fn noise_static_secret_bytes(&self) -> [u8; 32] {
        self.noise_static_secret
    }

    pub fn noise_public_key_bytes(&self) -> [u8; 32] {
        self.noise_public_key
    }

    pub fn set_device_name(&mut self, new_name: String, store: &SecureStore) -> Result<(), CoreError> {
        self.device_name = new_name;
        self.save_to_store(store)?;
        Ok(())
    }

    pub fn load_or_create(store: &SecureStore, default_name: Option<String>) -> Result<Self, CoreError> {
        if let Some(decrypted_bytes) = store.read_secure(IDENTITY_FILENAME)? {
            let stored: StoredIdentity = serde_json::from_slice(&decrypted_bytes)?;
            if stored.master_seed.len() == 32 {
                let mut seed = [0u8; 32];
                seed.copy_from_slice(&stored.master_seed);
                let id = Self::from_seed(&seed, stored.device_id, stored.device_name);
                seed.zeroize();
                return Ok(id);
            }
        }

        // Generate fresh identity and store it
        let identity = Self::new(default_name);
        identity.save_to_store(store)?;
        Ok(identity)
    }

    pub fn save_to_store(&self, store: &SecureStore) -> Result<(), CoreError> {
        let stored = StoredIdentity {
            device_id: self.device_id.clone(),
            device_name: self.device_name.clone(),
            master_seed: self.signing_key.to_bytes().to_vec(),
        };
        let json_bytes = serde_json::to_vec(&stored)?;
        store.write_secure(IDENTITY_FILENAME, &json_bytes)?;
        Ok(())
    }
}
