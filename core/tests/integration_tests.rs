use std::sync::Arc;
use tempfile::tempdir;
use tokio::io::duplex;

use localshare_core::identity::DeviceIdentity;
use localshare_core::pairing::{PairingAuthMode, PairingManager, MAX_FAILED_ATTEMPTS};
use localshare_core::secure_store::SecureStore;
use localshare_core::service::AppService;
use localshare_core::trust::{TrustedDevice, TrustStore};
use localshare_core::error::CoreError;

#[tokio::test]
async fn test_trust_store_crud() {
    let tmp = tempdir().unwrap();
    let store = Arc::new(SecureStore::new(tmp.path().to_path_buf()));
    let trust_store = TrustStore::new(store.clone());
    trust_store.load_or_init().await.unwrap();

    let id = DeviceIdentity::new(Some("Test Device".to_string()));
    let trusted = TrustedDevice::new(
        id.device_id.clone(),
        id.device_name.clone(),
        &id.verifying_key_bytes(),
        &id.noise_public_key_bytes(),
        id.fingerprint.clone(),
    );

    // Add
    trust_store.add_device(trusted.clone()).await.unwrap();
    assert!(trust_store.is_noise_key_trusted(&id.noise_public_key_bytes()).await);

    // List
    let list = trust_store.list_devices().await;
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].device_id, id.device_id);

    // Rename
    let renamed = trust_store.rename_device(&id.device_id, "Renamed Laptop").await.unwrap();
    assert_eq!(renamed.device_name, "Renamed Laptop");

    // Revoke
    let revoked = trust_store.revoke_device(&id.device_id).await.unwrap();
    assert_eq!(revoked.device_id, id.device_id);
    assert!(!trust_store.is_noise_key_trusted(&id.noise_public_key_bytes()).await);
    assert_eq!(trust_store.list_devices().await.len(), 0);
}

#[tokio::test]
async fn test_pairing_handshake_in_memory() {
    let tmp_a = tempdir().unwrap();
    let tmp_b = tempdir().unwrap();

    let store_a = Arc::new(SecureStore::new(tmp_a.path().to_path_buf()));
    let store_b = Arc::new(SecureStore::new(tmp_b.path().to_path_buf()));

    let id_a = DeviceIdentity::new(Some("Device A".to_string()));
    let id_b = DeviceIdentity::new(Some("Device B".to_string()));

    let trust_a = Arc::new(TrustStore::new(store_a));
    let trust_b = Arc::new(TrustStore::new(store_b));

    let pairing_mgr = PairingManager::new();
    let (secret_hex, _code, _exp) = pairing_mgr.create_session().await.unwrap();

    // In-memory duplex stream
    let (mut client_stream, mut server_stream) = duplex(4096);

    let id_a_clone = id_a.clone();
    let trust_a_clone = trust_a.clone();
    let server_handle = tokio::spawn(async move {
        pairing_mgr.handle_incoming_pairing(&mut server_stream, &id_a_clone, &trust_a_clone).await
    });

    let secret_bytes = hex::decode(secret_hex).unwrap();
    let id_b_clone = id_b.clone();
    let trust_b_clone = trust_b.clone();
    let client_handle = tokio::spawn(async move {
        PairingManager::initiate_pairing(
            &mut client_stream,
            &id_b_clone,
            PairingAuthMode::QrSecret,
            &secret_bytes,
            &trust_b_clone,
        ).await
    });

    let (res_a, res_b) = tokio::join!(server_handle, client_handle);
    let (trusted_b, sas_a) = res_a.unwrap().expect("Server pairing should succeed");
    let (trusted_a, sas_b) = res_b.unwrap().expect("Client pairing should succeed");

    assert_eq!(trusted_b.device_id, id_b.device_id);
    assert_eq!(trusted_a.device_id, id_a.device_id);

    // Crucial MITM check: SAS must match on both sides!
    assert_eq!(sas_a.numeric, sas_b.numeric);
    assert_eq!(sas_a.emoji, sas_b.emoji);

    // Both are now trusted
    assert!(trust_a.is_noise_key_trusted(&id_b.noise_public_key_bytes()).await);
    assert!(trust_b.is_noise_key_trusted(&id_a.noise_public_key_bytes()).await);
}

#[tokio::test]
async fn test_pairing_wrong_secret_fails() {
    let tmp_a = tempdir().unwrap();
    let tmp_b = tempdir().unwrap();

    let id_a = DeviceIdentity::new(Some("Device A".to_string()));
    let id_b = DeviceIdentity::new(Some("Device B".to_string()));

    let trust_a = Arc::new(TrustStore::new(Arc::new(SecureStore::new(tmp_a.path().to_path_buf()))));
    let trust_b = Arc::new(TrustStore::new(Arc::new(SecureStore::new(tmp_b.path().to_path_buf()))));

    let pairing_mgr = Arc::new(PairingManager::new());
    let (_secret_hex, _code, _exp) = pairing_mgr.create_session().await.unwrap();

    let (mut client_stream, mut server_stream) = duplex(4096);

    let id_a_clone = id_a.clone();
    let trust_a_clone = trust_a.clone();
    let pm = pairing_mgr.clone();
    let server_handle = tokio::spawn(async move {
        pm.handle_incoming_pairing(&mut server_stream, &id_a_clone, &trust_a_clone).await
    });

    let wrong_secret = [99u8; 32];
    let client_handle = tokio::spawn(async move {
        PairingManager::initiate_pairing(
            &mut client_stream,
            &id_b,
            PairingAuthMode::QrSecret,
            &wrong_secret,
            &trust_b,
        ).await
    });

    let (res_a, res_b) = tokio::join!(server_handle, client_handle);
    assert!(res_a.unwrap().is_err(), "Server must reject wrong secret");
    assert!(res_b.unwrap().is_err(), "Client must fail handshake with wrong secret");
}

#[tokio::test]
async fn test_pairing_rate_limiting() {
    let pairing_mgr = PairingManager::new();

    // Simulate 5 failed attempts
    for _ in 0..MAX_FAILED_ATTEMPTS {
        pairing_mgr.record_failure().await;
    }

    // Now attempt to create session or pair -> must be locked
    let err = pairing_mgr.create_session().await.unwrap_err();
    match err {
        CoreError::PairingRateLimited(secs) => {
            assert!(secs > 0 && secs <= 60);
        }
        other => panic!("Expected PairingRateLimited, got {:?}", other),
    }
}

#[tokio::test]
async fn test_in_process_network_peers_full_flow() {
    let tmp_a = tempdir().unwrap();
    let tmp_b = tempdir().unwrap();

    let svc_a = AppService::init(Some(tmp_a.path().to_path_buf())).await.unwrap();
    let svc_b = AppService::init(Some(tmp_b.path().to_path_buf())).await.unwrap();

    let port_a = svc_a.start_network_listener(0).await.unwrap();
    let _port_b = svc_b.start_network_listener(0).await.unwrap();

    // 1. Peer A starts pairing mode
    let qr_payload = svc_a.start_pairing_mode().await.unwrap();
    assert_eq!(qr_payload.port, port_a);

    // 2. Peer B pairs with Peer A using QR secret over real TCP
    let peer_addr = format!("127.0.0.1:{}", port_a);
    let (trusted_a_on_b, _sas_b) = svc_b
        .initiate_pairing_to_peer(&peer_addr, PairingAuthMode::QrSecret, &qr_payload.secret)
        .await
        .unwrap();

    // Verify Peer A identity matches
    let id_a = svc_a.get_identity().await;
    assert_eq!(trusted_a_on_b.device_id, id_a.device_id);
    assert_eq!(trusted_a_on_b.fingerprint, id_a.fingerprint);

    let id_b = svc_b.get_identity().await;
    // Check that Peer A also recorded Peer B (give spawned task a moment to persist)
    let mut trusted_devices_on_a = Vec::new();
    for _ in 0..20 {
        trusted_devices_on_a = svc_a.list_trusted_devices().await;
        if !trusted_devices_on_a.is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert_eq!(trusted_devices_on_a.len(), 1);
    assert_eq!(trusted_devices_on_a[0].device_id, id_b.device_id);

    // 3. Authenticated session & Ping
    let latency = svc_b.connect_and_ping(&peer_addr, &id_a.device_id).await.unwrap();
    println!("Ping latency between paired peers: {} ms", latency);

    // 4. Revocation test: Peer A revokes Peer B
    svc_a.revoke_trusted_device(&id_b.device_id).await.unwrap();
    assert_eq!(svc_a.list_trusted_devices().await.len(), 0);

    // 5. Subsequent connection from revoked Peer B must be dropped/rejected!
    let ping_after_revoke = svc_b.connect_and_ping(&peer_addr, &id_a.device_id).await;
    // On server side, Peer B's static key is no longer in trust store, connection is terminated
    assert!(ping_after_revoke.is_err(), "Revoked peer must not be allowed to connect");

    // 6. Unknown device test: Peer C connects without pairing
    let tmp_c = tempdir().unwrap();
    let svc_c = AppService::init(Some(tmp_c.path().to_path_buf())).await.unwrap();
    let ping_unknown = svc_c.connect_and_ping(&peer_addr, &id_a.device_id).await;
    assert!(ping_unknown.is_err(), "Unknown unpaired device must be rejected immediately");
}

#[tokio::test]
async fn test_pairing_expired_secret_fails() {
    let tmp_a = tempdir().unwrap();
    let tmp_b = tempdir().unwrap();

    let id_a = DeviceIdentity::new(Some("Device A".to_string()));
    let id_b = DeviceIdentity::new(Some("Device B".to_string()));

    let trust_a = Arc::new(TrustStore::new(Arc::new(SecureStore::new(tmp_a.path().to_path_buf()))));
    let trust_b = Arc::new(TrustStore::new(Arc::new(SecureStore::new(tmp_b.path().to_path_buf()))));

    let pairing_mgr = Arc::new(PairingManager::new());
    let (secret_hex, _code, _exp) = pairing_mgr.create_session().await.unwrap();

    // Invalidate/expire the session
    pairing_mgr.cancel_session().await;

    let (mut client_stream, mut server_stream) = duplex(4096);

    let id_a_clone = id_a.clone();
    let trust_a_clone = trust_a.clone();
    let pm = pairing_mgr.clone();
    let server_handle = tokio::spawn(async move {
        pm.handle_incoming_pairing(&mut server_stream, &id_a_clone, &trust_a_clone).await
    });

    let secret_bytes = hex::decode(secret_hex).unwrap();
    let client_handle = tokio::spawn(async move {
        PairingManager::initiate_pairing(
            &mut client_stream,
            &id_b,
            PairingAuthMode::QrSecret,
            &secret_bytes,
            &trust_b,
        ).await
    });

    let (res_a, _res_b) = tokio::join!(server_handle, client_handle);
    assert!(res_a.unwrap().is_err(), "Expired pairing session must be rejected");
}

