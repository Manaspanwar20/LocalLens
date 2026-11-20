use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::error::CoreError;
use crate::secure_store::SecureStore;

const TRUST_STORE_FILENAME: &str = "trust_store.enc";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrustedDevice {
    pub device_id: String,
    pub device_name: String,
    pub public_key_hex: String,
    pub noise_static_key_hex: String,
    pub fingerprint: String,
    pub paired_at: u64,
    pub last_seen: Option<u64>,
}

impl TrustedDevice {
    pub fn new(
        device_id: String,
        device_name: String,
        public_key: &[u8; 32],
        noise_static_key: &[u8; 32],
        fingerprint: String,
    ) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            device_id,
            device_name,
            public_key_hex: hex::encode(public_key),
            noise_static_key_hex: hex::encode(noise_static_key),
            fingerprint,
            paired_at: now,
            last_seen: Some(now),
        }
    }

    pub fn noise_static_key_bytes(&self) -> Result<[u8; 32], CoreError> {
        let bytes = hex::decode(&self.noise_static_key_hex)
            .map_err(|e| CoreError::Crypto(format!("Invalid noise static key hex: {}", e)))?;
        if bytes.len() != 32 {
            return Err(CoreError::Crypto("Invalid noise static key length".to_string()));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(arr)
    }

    pub fn public_key_bytes(&self) -> Result<[u8; 32], CoreError> {
        let bytes = hex::decode(&self.public_key_hex)
            .map_err(|e| CoreError::Crypto(format!("Invalid public key hex: {}", e)))?;
        if bytes.len() != 32 {
            return Err(CoreError::Crypto("Invalid public key length".to_string()));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(arr)
    }
}

pub struct TrustStore {
    devices: Arc<RwLock<HashMap<String, TrustedDevice>>>,
    store: Arc<SecureStore>,
}

impl TrustStore {
    pub fn new(store: Arc<SecureStore>) -> Self {
        Self {
            devices: Arc::new(RwLock::new(HashMap::new())),
            store,
        }
    }

    pub async fn load_or_init(&self) -> Result<(), CoreError> {
        if let Some(decrypted_bytes) = self.store.read_secure(TRUST_STORE_FILENAME)? {
            let list: Vec<TrustedDevice> = serde_json::from_slice(&decrypted_bytes)?;
            let mut map = self.devices.write().await;
            for dev in list {
                map.insert(dev.device_id.clone(), dev);
            }
        }
        Ok(())
    }

    pub async fn save(&self) -> Result<(), CoreError> {
        let list: Vec<TrustedDevice> = {
            let map = self.devices.read().await;
            map.values().cloned().collect()
        };
        let bytes = serde_json::to_vec(&list)?;
        self.store.write_secure(TRUST_STORE_FILENAME, &bytes)?;
        Ok(())
    }

    pub async fn is_noise_key_trusted(&self, noise_static_key: &[u8; 32]) -> bool {
        let hex_key = hex::encode(noise_static_key);
        let map = self.devices.read().await;
        map.values().any(|d| d.noise_static_key_hex == hex_key)
    }

    pub async fn get_by_noise_key(&self, noise_static_key: &[u8; 32]) -> Option<TrustedDevice> {
        let hex_key = hex::encode(noise_static_key);
        let map = self.devices.read().await;
        map.values().find(|d| d.noise_static_key_hex == hex_key).cloned()
    }

    pub async fn get_by_device_id(&self, device_id: &str) -> Option<TrustedDevice> {
        let map = self.devices.read().await;
        map.get(device_id).cloned()
    }

    pub async fn list_devices(&self) -> Vec<TrustedDevice> {
        let map = self.devices.read().await;
        let mut list: Vec<TrustedDevice> = map.values().cloned().collect();
        list.sort_by(|a, b| b.paired_at.cmp(&a.paired_at));
        list
    }

    pub async fn add_device(&self, device: TrustedDevice) -> Result<(), CoreError> {
        {
            let mut map = self.devices.write().await;
            map.insert(device.device_id.clone(), device);
        }
        self.save().await?;
        Ok(())
    }

    pub async fn rename_device(&self, device_id: &str, new_name: &str) -> Result<TrustedDevice, CoreError> {
        let updated = {
            let mut map = self.devices.write().await;
            if let Some(dev) = map.get_mut(device_id) {
                dev.device_name = new_name.to_string();
                dev.clone()
            } else {
                return Err(CoreError::DeviceNotFound(device_id.to_string()));
            }
        };
        self.save().await?;
        Ok(updated)
    }

    pub async fn revoke_device(&self, device_id: &str) -> Result<TrustedDevice, CoreError> {
        let revoked = {
            let mut map = self.devices.write().await;
            map.remove(device_id)
                .ok_or_else(|| CoreError::DeviceNotFound(device_id.to_string()))?
        };
        self.save().await?;
        Ok(revoked)
    }

    pub async fn update_last_seen(&self, device_id: &str) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let mut map = self.devices.write().await;
        if let Some(dev) = map.get_mut(device_id) {
            dev.last_seen = Some(now);
        }
    }
}
