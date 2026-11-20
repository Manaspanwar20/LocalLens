use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, RwLock};
use tracing::info;

use crate::error::CoreError;
use crate::trust::TrustStore;

pub const SERVICE_TYPE: &str = "_localshare._tcp.local.";
pub const SCAN_DURATION_SECS: u64 = 10;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum DeviceStatus {
    Paired,
    NotPaired,
    Online,
    Offline,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiscoveredDevice {
    pub device_id: String,
    pub device_name: String,
    pub protocol_version: String,
    pub pairing_mode: bool,
    pub fingerprint: String,
    pub addresses: Vec<String>,
    pub port: u16,
    pub status: DeviceStatus,
    pub last_seen: u64,
}

pub struct DiscoveryManager {
    daemon: ServiceDaemon,
    registered_fullname: RwLock<Option<String>>,
    discovered_cache: Arc<RwLock<HashMap<String, DiscoveredDevice>>>,
    event_tx: broadcast::Sender<Vec<DiscoveredDevice>>,
    is_scanning: Arc<RwLock<bool>>,
}

impl DiscoveryManager {
    pub fn new() -> Result<Self, CoreError> {
        let daemon = ServiceDaemon::new()
            .map_err(|e| CoreError::Discovery(format!("Failed to initialize mDNS daemon: {}. Check Windows Firewall permissions.", e)))?;
        let (event_tx, _) = broadcast::channel(32);

        Ok(Self {
            daemon,
            registered_fullname: RwLock::new(None),
            discovered_cache: Arc::new(RwLock::new(HashMap::new())),
            event_tx,
            is_scanning: Arc::new(RwLock::new(false)),
        })
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Vec<DiscoveredDevice>> {
        self.event_tx.subscribe()
    }

    pub async fn start_advertising(
        &self,
        device_id: &str,
        device_name: &str,
        port: u16,
        pairing_mode: bool,
        fingerprint: &str,
    ) -> Result<(), CoreError> {
        let instance_name = format!("{}-{}", sanitize_instance_name(device_name), &device_id[..6]);
        let host_name = format!("{}.local.", instance_name);

        let mut properties = HashMap::new();
        properties.insert("device_id".to_string(), device_id.to_string());
        properties.insert("device_name".to_string(), device_name.to_string());
        properties.insert("protocol_version".to_string(), "1".to_string());
        properties.insert(
            "pairing_mode".to_string(),
            if pairing_mode { "1".to_string() } else { "0".to_string() },
        );
        properties.insert("fingerprint".to_string(), fingerprint.to_string());

        let my_service = ServiceInfo::new(
            SERVICE_TYPE,
            &instance_name,
            &host_name,
            "",
            port,
            properties,
        ).map_err(|e| CoreError::Discovery(format!("Invalid mDNS service info: {}", e)))?;

        let fullname = my_service.get_fullname().to_string();
        self.daemon
            .register(my_service)
            .map_err(|e| CoreError::Discovery(format!("Failed to register mDNS service: {}. Check Windows Firewall settings.", e)))?;

        *self.registered_fullname.write().await = Some(fullname);
        info!("mDNS advertisement started on port {}", port);
        Ok(())
    }

    pub async fn update_pairing_mode(
        &self,
        device_id: &str,
        device_name: &str,
        port: u16,
        pairing_mode: bool,
        fingerprint: &str,
    ) -> Result<(), CoreError> {
        // Unregister old if exists
        let old_name = self.registered_fullname.read().await.clone();
        if let Some(name) = old_name {
            let _ = self.daemon.unregister(&name);
        }

        self.start_advertising(device_id, device_name, port, pairing_mode, fingerprint).await
    }

    pub async fn stop_advertising(&self) {
        let old_name = self.registered_fullname.write().await.take();
        if let Some(name) = old_name {
            let _ = self.daemon.unregister(&name);
            info!("mDNS advertisement unregistered");
        }
    }

    pub async fn get_discovered_devices(&self) -> Vec<DiscoveredDevice> {
        let cache = self.discovered_cache.read().await;
        cache.values().cloned().collect()
    }

    pub async fn start_scan(
        &self,
        trust_store: Arc<TrustStore>,
        own_device_id: String,
    ) -> Result<(), CoreError> {
        {
            let mut scanning = self.is_scanning.write().await;
            if *scanning {
                return Ok(());
            }
            *scanning = true;
        }

        let receiver = self.daemon
            .browse(SERVICE_TYPE)
            .map_err(|e| CoreError::Discovery(format!("Failed to browse mDNS services: {}. Check Windows Firewall permissions.", e)))?;

        let cache = self.discovered_cache.clone();
        let event_tx = self.event_tx.clone();
        let is_scanning = self.is_scanning.clone();

        tokio::spawn(async move {
            info!("Starting mDNS scan for ~{} seconds", SCAN_DURATION_SECS);
            let start_time = Instant::now();
            let scan_duration = Duration::from_secs(SCAN_DURATION_SECS);

            while start_time.elapsed() < scan_duration {
                // Poll events with timeout
                let event_opt = tokio::task::spawn_blocking({
                    let recv = receiver.clone();
                    move || recv.recv_timeout(Duration::from_millis(300))
                }).await.unwrap_or_else(|_| Err(flume::RecvTimeoutError::Timeout));

                match event_opt {
                    Ok(event) => match event {
                        ServiceEvent::ServiceResolved(info) => {
                            let props = info.get_properties();
                            let device_id = props.get_property_val_str("device_id")
                                .unwrap_or_default()
                                .to_string();

                            // Skip self
                            if device_id.is_empty() || device_id == own_device_id {
                                continue;
                            }

                            let device_name = props.get_property_val_str("device_name")
                                .unwrap_or_else(|| info.get_fullname())
                                .to_string();
                            let protocol_version = props.get_property_val_str("protocol_version")
                                .unwrap_or("1")
                                .to_string();
                            let pairing_mode = props.get_property_val_str("pairing_mode")
                                .map(|v| v == "1")
                                .unwrap_or(false);
                            let fingerprint = props.get_property_val_str("fingerprint")
                                .unwrap_or_default()
                                .to_string();

                            let addresses: Vec<String> = info.get_addresses()
                                .iter()
                                .filter(|ip| match ip {
                                    IpAddr::V4(v4) => !v4.is_loopback() && !v4.is_link_local(),
                                    IpAddr::V6(v6) => !v6.is_loopback(),
                                })
                                .map(|ip| ip.to_string())
                                .collect();

                            let port = info.get_port();
                            let now = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs();

                            let is_paired = trust_store.get_by_device_id(&device_id).await.is_some();
                            let status = if is_paired {
                                DeviceStatus::Paired
                            } else {
                                DeviceStatus::NotPaired
                            };

                            let dev = DiscoveredDevice {
                                device_id: device_id.clone(),
                                device_name,
                                protocol_version,
                                pairing_mode,
                                fingerprint,
                                addresses,
                                port,
                                status,
                                last_seen: now,
                            };

                            {
                                let mut map = cache.write().await;
                                map.insert(device_id, dev);
                            }

                            let list: Vec<DiscoveredDevice> = cache.read().await.values().cloned().collect();
                            let _ = event_tx.send(list);
                        }
                        ServiceEvent::ServiceRemoved(_, fullname) => {
                            let mut map = cache.write().await;
                            map.retain(|_, v| !fullname.contains(&v.device_name));
                            let list: Vec<DiscoveredDevice> = map.values().cloned().collect();
                            let _ = event_tx.send(list);
                        }
                        _ => {}
                    },
                    Err(flume::RecvTimeoutError::Timeout) => {
                        // Just loop
                    }
                    Err(flume::RecvTimeoutError::Disconnected) => {
                        break;
                    }
                }
            }

            // Scan completed
            *is_scanning.write().await = false;
            info!("mDNS scan completed");
        });

        Ok(())
    }
}

fn sanitize_instance_name(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .take(15)
        .collect()
}
