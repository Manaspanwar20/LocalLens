use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use serde::{Deserialize, Serialize};
use tokio::net::UdpSocket;
use tokio::sync::{broadcast, RwLock};
use tracing::{info, warn};

use crate::error::CoreError;
use crate::trust::TrustStore;

pub const SERVICE_TYPE: &str = "_localshare._tcp.local.";
pub const SCAN_DURATION_SECS: u64 = 10;
pub const UDP_DISCOVERY_PORT: u16 = 52481;
pub const UDP_BEACON_MAGIC: &str = "LSHR_BEACON_V1";

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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiscoveryBeacon {
    pub magic: String,
    pub version: u8,
    pub device_id: String,
    pub device_name: String,
    pub port: u16,
    pub pairing_mode: bool,
    pub fingerprint: String,
    pub is_query: bool,
}

pub struct DiscoveryManager {
    daemon: Option<ServiceDaemon>,
    registered_fullname: RwLock<Option<String>>,
    discovered_cache: Arc<RwLock<HashMap<String, DiscoveredDevice>>>,
    event_tx: broadcast::Sender<Vec<DiscoveredDevice>>,
    is_scanning: Arc<RwLock<bool>>,
    udp_socket: Arc<UdpSocket>,
    advertising_beacon: Arc<RwLock<Option<DiscoveryBeacon>>>,
    trust_store: Arc<RwLock<Option<Arc<TrustStore>>>>,
}

impl DiscoveryManager {
    pub async fn new() -> Result<Self, CoreError> {
        let daemon = ServiceDaemon::new().ok();
        let (event_tx, _) = broadcast::channel(64);
        let discovered_cache = Arc::new(RwLock::new(HashMap::new()));
        let is_scanning = Arc::new(RwLock::new(false));
        let advertising_beacon = Arc::new(RwLock::new(None));
        let trust_store_holder: Arc<RwLock<Option<Arc<TrustStore>>>> = Arc::new(RwLock::new(None));

        // Bind UDP socket on port 52481 (or fallback to ephemeral 0 if busy)
        let udp_socket = match UdpSocket::bind(format!("0.0.0.0:{}", UDP_DISCOVERY_PORT)).await {
            Ok(s) => {
                let _ = s.set_broadcast(true);
                Arc::new(s)
            }
            Err(_) => {
                let s = UdpSocket::bind("0.0.0.0:0").await
                    .map_err(|e| CoreError::Discovery(format!("Failed to bind UDP socket: {}", e)))?;
                let _ = s.set_broadcast(true);
                Arc::new(s)
            }
        };

        // Spawn background UDP listener
        let sock_rx = udp_socket.clone();
        let cache_clone = discovered_cache.clone();
        let tx_clone = event_tx.clone();
        let adv_clone = advertising_beacon.clone();
        let ts_clone = trust_store_holder.clone();

        tokio::spawn(async move {
            let mut buf = [0u8; 2048];
            loop {
                match sock_rx.recv_from(&mut buf).await {
                    Ok((len, peer_addr)) => {
                        if let Ok(beacon) = serde_json::from_slice::<DiscoveryBeacon>(&buf[..len]) {
                            if beacon.magic == UDP_BEACON_MAGIC {
                                let sender_ip = peer_addr.ip().to_string();

                                // If this is a query and we are advertising, send our beacon back to the sender
                                if beacon.is_query {
                                    let my_adv = adv_clone.read().await.clone();
                                    if let Some(my_beacon) = my_adv {
                                        if let Ok(reply_bytes) = serde_json::to_vec(&my_beacon) {
                                            let _ = sock_rx.send_to(&reply_bytes, peer_addr).await;
                                        }
                                    }
                                }

                                // If beacon has device info (not just empty query), record it
                                if !beacon.device_id.is_empty() && beacon.port > 0 {
                                    let now = SystemTime::now()
                                        .duration_since(UNIX_EPOCH)
                                        .unwrap_or_default()
                                        .as_secs();

                                    let is_paired = if let Some(ts) = ts_clone.read().await.as_ref() {
                                        ts.get_by_device_id(&beacon.device_id).await.is_some()
                                    } else {
                                        false
                                    };

                                    let status = if is_paired {
                                        DeviceStatus::Paired
                                    } else {
                                        DeviceStatus::NotPaired
                                    };

                                    let status_for_insert = status.clone();
                                    let mut map = cache_clone.write().await;
                                    let entry = map.entry(beacon.device_id.clone()).or_insert_with(|| DiscoveredDevice {
                                        device_id: beacon.device_id.clone(),
                                        device_name: beacon.device_name.clone(),
                                        protocol_version: beacon.version.to_string(),
                                        pairing_mode: beacon.pairing_mode,
                                        fingerprint: beacon.fingerprint.clone(),
                                        addresses: Vec::new(),
                                        port: beacon.port,
                                        status: status_for_insert,
                                        last_seen: now,
                                    });

                                    entry.device_name = beacon.device_name;
                                    entry.port = beacon.port;
                                    entry.pairing_mode = beacon.pairing_mode;
                                    entry.fingerprint = beacon.fingerprint;
                                    entry.last_seen = now;
                                    entry.status = status;
                                    if !entry.addresses.contains(&sender_ip) {
                                        entry.addresses.push(sender_ip);
                                    }

                                    let all: Vec<DiscoveredDevice> = map.values().cloned().collect();
                                    drop(map);
                                    let _ = tx_clone.send(all);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!("UDP discovery recv error: {}", e);
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }
            }
        });

        Ok(Self {
            daemon,
            registered_fullname: RwLock::new(None),
            discovered_cache,
            event_tx,
            is_scanning,
            udp_socket,
            advertising_beacon,
            trust_store: trust_store_holder,
        })
    }

    pub fn set_trust_store(&self, store: Arc<TrustStore>) {
        let holder = self.trust_store.clone();
        tokio::spawn(async move {
            *holder.write().await = Some(store);
        });
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
        let beacon = DiscoveryBeacon {
            magic: UDP_BEACON_MAGIC.to_string(),
            version: 1,
            device_id: device_id.to_string(),
            device_name: device_name.to_string(),
            port,
            pairing_mode,
            fingerprint: fingerprint.to_string(),
            is_query: false,
        };

        *self.advertising_beacon.write().await = Some(beacon.clone());

        // 1. Send immediate UDP broadcast
        if let Ok(bytes) = serde_json::to_vec(&beacon) {
            let _ = self.udp_socket.send_to(&bytes, format!("255.255.255.255:{}", UDP_DISCOVERY_PORT)).await;
        }

        // 2. Start mDNS advertisement if daemon is available
        if let Some(daemon) = &self.daemon {
            let instance_name = format!("{}-{}", sanitize_instance_name(device_name), &device_id[..6.min(device_id.len())]);
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

            if let Ok(my_service) = ServiceInfo::new(
                SERVICE_TYPE,
                &instance_name,
                &host_name,
                "",
                port,
                properties,
            ) {
                let fullname = my_service.get_fullname().to_string();
                let _ = daemon.register(my_service);
                *self.registered_fullname.write().await = Some(fullname);
            }
        }

        info!("LocalShare discovery advertising active on port {}", port);
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
        // Unregister mDNS if exists
        let old_name = self.registered_fullname.read().await.clone();
        if let Some(name) = old_name {
            if let Some(d) = &self.daemon {
                let _ = d.unregister(&name);
            }
        }

        self.start_advertising(device_id, device_name, port, pairing_mode, fingerprint).await
    }

    pub async fn stop_advertising(&self) {
        *self.advertising_beacon.write().await = None;
        if let Some(daemon) = &self.daemon {
            let old_name = self.registered_fullname.write().await.take();
            if let Some(name) = old_name {
                let _ = daemon.unregister(&name);
            }
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
        self.set_trust_store(trust_store.clone());

        {
            let mut scanning = self.is_scanning.write().await;
            if *scanning {
                return Ok(());
            }
            *scanning = true;
        }

        // ── 1. UDP Broadcast & Subnet Sweep ───────────────────────────
        let socket = self.udp_socket.clone();
        let my_id = own_device_id.clone();
        tokio::spawn(async move {
            let query_beacon = DiscoveryBeacon {
                magic: UDP_BEACON_MAGIC.to_string(),
                version: 1,
                device_id: my_id.clone(),
                device_name: String::new(),
                port: 0,
                pairing_mode: false,
                fingerprint: String::new(),
                is_query: true,
            };

            if let Ok(bytes) = serde_json::to_vec(&query_beacon) {
                // Broadcast to standard port
                let _ = socket.send_to(&bytes, format!("255.255.255.255:{}", UDP_DISCOVERY_PORT)).await;

                // Subnet sweep: for all local IPv4 network interfaces, ping all 254 hosts on port 52481
                if let Ok(ifaces) = if_addrs::get_if_addrs() {
                    for iface in ifaces {
                        if !iface.is_loopback() {
                            if let IpAddr::V4(v4) = iface.ip() {
                                let oct = v4.octets();
                                for host in 1..=254 {
                                    if host != oct[3] {
                                        let target = format!("{}.{}.{}.{}:{}", oct[0], oct[1], oct[2], host, UDP_DISCOVERY_PORT);
                                        let _ = socket.send_to(&bytes, &target).await;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });

        // ── 2. mDNS Scan (if available) ───────────────────────────────
        let daemon_opt = self.daemon.clone();
        let cache = self.discovered_cache.clone();
        let event_tx = self.event_tx.clone();
        let is_scanning = self.is_scanning.clone();

        tokio::spawn(async move {
            info!("Starting Wi-Fi discovery scan for ~{} seconds", SCAN_DURATION_SECS);

            if let Some(daemon) = daemon_opt {
                if let Ok(receiver) = daemon.browse(SERVICE_TYPE) {
                    let start_time = Instant::now();
                    let scan_duration = Duration::from_secs(SCAN_DURATION_SECS);

                    while start_time.elapsed() < scan_duration {
                        let event_opt = tokio::task::spawn_blocking({
                            let recv = receiver.clone();
                            move || recv.recv_timeout(Duration::from_millis(300))
                        }).await.unwrap_or_else(|_| Err(flume::RecvTimeoutError::Timeout));

                        match event_opt {
                            Ok(ServiceEvent::ServiceResolved(info)) => {
                                let props = info.get_properties();
                                let device_id = props.get_property_val_str("device_id")
                                    .unwrap_or_default()
                                    .to_string();

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
                            Ok(ServiceEvent::ServiceRemoved(_, fullname)) => {
                                let mut map = cache.write().await;
                                map.retain(|_, v| !fullname.contains(&v.device_name));
                                let list: Vec<DiscoveredDevice> = map.values().cloned().collect();
                                let _ = event_tx.send(list);
                            }
                            _ => {}
                        }
                    }
                }
            } else {
                tokio::time::sleep(Duration::from_secs(SCAN_DURATION_SECS)).await;
            }

            *is_scanning.write().await = false;
            info!("Wi-Fi scan completed");
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
