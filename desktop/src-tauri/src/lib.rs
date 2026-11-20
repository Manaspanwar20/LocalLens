use std::sync::Arc;
use tauri::{Emitter, Manager, State};

use localshare_core::discovery::DiscoveredDevice;
use localshare_core::pairing::{PairingAuthMode, QrPairingPayload, SasConfirmation};
use localshare_core::service::{AppService, IdentityDto};
use localshare_core::trust::TrustedDevice;

struct AppState {
    service: Arc<AppService>,
}

#[tauri::command]
async fn get_identity(state: State<'_, AppState>) -> Result<IdentityDto, String> {
    Ok(state.service.get_identity().await)
}

#[tauri::command]
async fn update_device_name(name: String, state: State<'_, AppState>) -> Result<(), String> {
    state.service.update_device_name(name).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn start_discovery(state: State<'_, AppState>) -> Result<(), String> {
    state.service.start_discovery_scan().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_discovered_devices(state: State<'_, AppState>) -> Result<Vec<DiscoveredDevice>, String> {
    Ok(state.service.discovery.get_discovered_devices().await)
}

#[tauri::command]
async fn start_pairing(state: State<'_, AppState>) -> Result<QrPairingPayload, String> {
    state.service.start_pairing_mode().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn cancel_pairing(state: State<'_, AppState>) -> Result<(), String> {
    state.service.cancel_pairing_mode().await.map_err(|e| e.to_string())
}

#[derive(serde::Serialize)]
struct PairingResult {
    peer: TrustedDevice,
    sas: SasConfirmation,
}

#[tauri::command]
async fn initiate_pairing(
    peer_addr: String,
    mode: String,
    secret_or_code: String,
    state: State<'_, AppState>,
) -> Result<PairingResult, String> {
    let auth_mode = match mode.as_str() {
        "code" => PairingAuthMode::NumericCode,
        _ => PairingAuthMode::QrSecret,
    };

    let (peer, sas) = state
        .service
        .initiate_pairing_to_peer(&peer_addr, auth_mode, &secret_or_code)
        .await
        .map_err(|e| e.to_string())?;

    Ok(PairingResult { peer, sas })
}

#[tauri::command]
async fn list_trusted_devices(state: State<'_, AppState>) -> Result<Vec<TrustedDevice>, String> {
    Ok(state.service.list_trusted_devices().await)
}

#[tauri::command]
async fn rename_trusted_device(
    device_id: String,
    new_name: String,
    state: State<'_, AppState>,
) -> Result<TrustedDevice, String> {
    state
        .service
        .rename_trusted_device(&device_id, &new_name)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn revoke_trusted_device(
    device_id: String,
    state: State<'_, AppState>,
) -> Result<TrustedDevice, String> {
    state
        .service
        .revoke_trusted_device(&device_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn ping_peer(
    peer_addr: String,
    device_id: String,
    state: State<'_, AppState>,
) -> Result<u64, String> {
    state
        .service
        .connect_and_ping(&peer_addr, &device_id)
        .await
        .map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let handle = app.handle().clone();

            // Initialize tokio runtime for AppService
            tauri::async_runtime::block_on(async move {
                let service = match AppService::init(None).await {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("Failed to init AppService: {}", e);
                        panic!("Failed to init AppService: {}", e);
                    }
                };

                // Start network listener on random port or 0
                if let Err(e) = service.start_network_listener(0).await {
                    eprintln!("Failed to start network listener: {}", e);
                }

                // Subscribe to service events and forward to Tauri frontend
                let mut rx = service.subscribe();
                let event_handle = handle.clone();
                tauri::async_runtime::spawn(async move {
                    while let Ok(evt) = rx.recv().await {
                        let _ = event_handle.emit("service_event", evt);
                    }
                });

                // Also forward discovery stream
                let mut disc_rx = service.discovery.subscribe();
                let disc_handle = handle.clone();
                tauri::async_runtime::spawn(async move {
                    while let Ok(devices) = disc_rx.recv().await {
                        let _ = disc_handle.emit("discovered_devices", devices);
                    }
                });

                handle.manage(AppState { service });
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_identity,
            update_device_name,
            start_discovery,
            get_discovered_devices,
            start_pairing,
            cancel_pairing,
            initiate_pairing,
            list_trusted_devices,
            rename_trusted_device,
            revoke_trusted_device,
            ping_peer,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
