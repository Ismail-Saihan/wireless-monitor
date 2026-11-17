//! Unit and integration tests for mDNS capability broadcast & reverse monitor initiation ("Use as Monitor")
//!
//! Tests:
//! 1. PeerDevice serialization & deserialization with capabilities, max_resolution, max_fps, last_seen.
//! 2. Backward compatibility: deserialization of legacy JSON lacking capabilities defaults safely.
//! 3. MdnsDiscovery::start_advertising_with_caps publishes RFC-compliant capability TXT properties.
//! 4. TXT record parsing extracts caps, res, and fps with fallback defaults (1080p, 60fps).
//! 5. Simulated peer discovery event propagation with capability attributes.
//! 6. initiate_use_as_monitor workflow sets app role to receiver and enters pairing state.
//! 7. initiate_use_as_monitor input validation (rejects invalid geometry and empty peer IDs).

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use wireless_monitor_lib::discovery::{
    parse_txt_capabilities, sanitize_dns_label, DiscoveryService, MdnsDiscovery, PeerDevice,
    SERVICE_TYPE,
};
use wireless_monitor_lib::receiver::ClientScreenGeometry;
use wireless_monitor_lib::{AppState, SessionStatus};

#[test]
fn test_peer_device_serialization_with_capabilities() {
    let peer = PeerDevice {
        id: "peer-receiver-01".to_string(),
        name: "Dell-Laptop-Monitor".to_string(),
        ip: IpAddr::V4(Ipv4Addr::new(192, 168, 1, 120)),
        port: 5000,
        mode: "monitor".to_string(),
        last_seen: 1700000000,
        capabilities: vec!["1080p".to_string(), "1440p".to_string(), "4k".to_string()],
        max_resolution: "2560x1440".to_string(),
        max_fps: 60,
    };

    assert!(peer.is_monitor());
    assert!(!peer.is_host());
    assert_eq!(peer.max_resolution, "2560x1440");
    assert_eq!(peer.max_fps, 60);
    assert_eq!(peer.capabilities.len(), 3);

    let json = serde_json::to_string(&peer).expect("Serialization must succeed");
    assert!(json.contains("peer-receiver-01"));
    assert!(json.contains("Dell-Laptop-Monitor"));
    assert!(json.contains("192.168.1.120"));
    assert!(json.contains("2560x1440"));
    assert!(json.contains("1440p"));
    assert!(json.contains("60"));

    let deserialized: PeerDevice =
        serde_json::from_str(&json).expect("Deserialization must succeed");
    assert_eq!(deserialized.id, peer.id);
    assert_eq!(deserialized.name, peer.name);
    assert_eq!(deserialized.ip, peer.ip);
    assert_eq!(deserialized.port, peer.port);
    assert_eq!(deserialized.mode, peer.mode);
    assert_eq!(deserialized.last_seen, peer.last_seen);
    assert_eq!(deserialized.capabilities, peer.capabilities);
    assert_eq!(deserialized.max_resolution, peer.max_resolution);
    assert_eq!(deserialized.max_fps, peer.max_fps);
}

#[test]
fn test_peer_device_legacy_backward_compatibility() {
    // Legacy JSON from Phase 1/Phase 2 without capabilities, max_resolution, max_fps, last_seen
    let legacy_json = r#"{
        "id": "legacy-host-01",
        "name": "Old-Desktop",
        "ip": "192.168.1.45",
        "port": 5000,
        "mode": "host"
    }"#;

    let peer: PeerDevice =
        serde_json::from_str(legacy_json).expect("Legacy PeerDevice JSON must deserialize cleanly");

    assert_eq!(peer.id, "legacy-host-01");
    assert_eq!(peer.name, "Old-Desktop");
    assert_eq!(peer.ip, IpAddr::V4(Ipv4Addr::new(192, 168, 1, 45)));
    assert_eq!(peer.port, 5000);
    assert_eq!(peer.mode, "host");
    assert!(peer.is_host());

    // Defaults must be safe 1080p @ 60fps
    assert_eq!(peer.max_resolution, "1920x1080");
    assert_eq!(peer.max_fps, 60);
    assert!(peer.capabilities.contains(&"1080p".to_string()));
    assert_eq!(peer.last_seen, 0);
}

#[test]
fn test_mdns_advertising_with_capabilities() {
    let discovery = MdnsDiscovery::new_simulated();
    assert!(discovery.is_simulated());
    assert!(!discovery.is_advertising());

    let caps = vec!["1080p".to_string(), "1440p".to_string()];
    let res = discovery.start_advertising_with_caps(
        "Office-PC",
        5000,
        "host",
        &caps,
        "2560x1440",
        60,
    );
    assert!(res.is_ok(), "Advertising with capabilities must succeed");
    assert!(discovery.is_advertising());

    let info = discovery
        .get_advertised_info()
        .expect("Advertised info must be present");
    assert_eq!(info.0, "Office-PC");
    assert_eq!(info.1, 5000);
    assert_eq!(info.2, "host");

    let advertised_caps = discovery
        .get_advertised_caps()
        .expect("Advertised caps must be present");
    assert_eq!(advertised_caps.0, caps);
    assert_eq!(advertised_caps.1, "2560x1440");
    assert_eq!(advertised_caps.2, 60);

    let stop_res = discovery.stop();
    assert!(stop_res.is_ok());
    assert!(!discovery.is_advertising());
}

#[test]
fn test_txt_record_capability_parsing() {
    // 1. Full TXT records
    let mut props = HashMap::new();
    props.insert("caps".to_string(), "1080p,1440p,4k".to_string());
    props.insert("res".to_string(), "3840x2160".to_string());
    props.insert("fps".to_string(), "60".to_string());

    let (caps, res, fps) = parse_txt_capabilities(&props);
    assert_eq!(caps, vec!["1080p", "1440p", "4k"]);
    assert_eq!(res, "3840x2160");
    assert_eq!(fps, 60);

    // 2. Legacy / empty TXT records defaults
    let empty_props = HashMap::new();
    let (def_caps, def_res, def_fps) = parse_txt_capabilities(&empty_props);
    assert_eq!(def_caps, vec!["1080p"]);
    assert_eq!(def_res, "1920x1080");
    assert_eq!(def_fps, 60);

    // 3. Alternate property names (capabilities, max_resolution, max_fps)
    let mut alt_props = HashMap::new();
    alt_props.insert("capabilities".to_string(), "720p, 1080p".to_string());
    alt_props.insert("max_resolution".to_string(), "1280x720".to_string());
    alt_props.insert("max_fps".to_string(), "30".to_string());

    let (alt_caps, alt_res, alt_fps) = parse_txt_capabilities(&alt_props);
    assert_eq!(alt_caps, vec!["720p", "1080p"]);
    assert_eq!(alt_res, "1280x720");
    assert_eq!(alt_fps, 30);
}

#[test]
fn test_simulated_peer_discovery_with_caps_event() {
    let discovery = MdnsDiscovery::new_simulated();
    let peer_found = Arc::new(AtomicBool::new(false));
    let peer_found_clone = Arc::clone(&peer_found);

    let browse_res = discovery.start_browsing(move |peer: PeerDevice| {
        if peer.id == "target-peer-99" {
            assert_eq!(peer.max_resolution, "2560x1440");
            assert_eq!(peer.max_fps, 60);
            assert!(peer.capabilities.contains(&"1440p".to_string()));
            peer_found_clone.store(true, Ordering::SeqCst);
        }
    });
    assert!(browse_res.is_ok());

    let test_peer = PeerDevice {
        id: "target-peer-99".to_string(),
        name: "Studio-MacBook".to_string(),
        ip: IpAddr::V4(Ipv4Addr::new(192, 168, 1, 88)),
        port: 5000,
        mode: "monitor".to_string(),
        last_seen: 1700000500,
        capabilities: vec!["1080p".to_string(), "1440p".to_string()],
        max_resolution: "2560x1440".to_string(),
        max_fps: 60,
    };

    let sim_res = discovery.simulate_peer_discovered(test_peer);
    assert!(sim_res.is_ok());
    assert!(peer_found.load(Ordering::SeqCst));

    let peers = discovery.get_discovered_peers();
    assert_eq!(peers.len(), 1);
    assert_eq!(peers[0].max_resolution, "2560x1440");
    assert_eq!(peers[0].capabilities, vec!["1080p", "1440p"]);
}

#[test]
fn test_initiate_use_as_monitor_workflow() {
    let app_state = AppState::new_simulated();

    // Receiver initiates reverse pulling to act as monitor for remote host
    let geometry = ClientScreenGeometry::new(2560, 1440, 60);
    let session = app_state
        .initiate_use_as_monitor("target-host-device", &geometry)
        .expect("initiate_use_as_monitor must succeed for valid target and geometry");

    assert_eq!(session.peer_id, "target-host-device");
    assert_eq!(session.state, "pairing");
    assert_eq!(session.resolution, Some("2560x1440".to_string()));
    assert_eq!(session.fps, Some(60));
    assert!(session.pin.is_some(), "PIN challenge must be generated");

    let pin = session.pin.unwrap();
    assert_eq!(pin.len(), 4);
    assert!(pin.chars().all(|c| c.is_ascii_digit()));

    // Active session in state must match
    let active = app_state
        .get_active_session()
        .expect("Active session must be set in AppState");
    assert_eq!(active.peer_id, "target-host-device");
    assert_eq!(active.state, "pairing");

    // Local role must transition to receiver
    let role = app_state
        .app_role
        .lock()
        .map(|r| r.clone())
        .unwrap_or_default();
    assert_eq!(role, "receiver");
}

#[test]
fn test_initiate_use_as_monitor_invalid_inputs_rejected() {
    let app_state = AppState::new_simulated();

    // 1. Empty peer ID must be rejected
    let valid_geom = ClientScreenGeometry::new(1920, 1080, 60);
    let err_empty_id = app_state.initiate_use_as_monitor("   ", &valid_geom);
    assert!(err_empty_id.is_err());
    assert!(err_empty_id.unwrap_err().contains("empty"));

    // 2. Invalid screen geometry dimensions (zero dimensions)
    let invalid_geom = ClientScreenGeometry::new(0, 0, 60);
    let err_zero = app_state.initiate_use_as_monitor("host-pc-1", &invalid_geom);
    assert!(err_zero.is_err());
    assert!(err_zero.unwrap_err().contains("Invalid receiver geometry"));

    // 3. Odd resolution dimensions (odd width/height invalid for H.264 macroblocks)
    let odd_geom = ClientScreenGeometry::new(1921, 1080, 60);
    let err_odd = app_state.initiate_use_as_monitor("host-pc-1", &odd_geom);
    assert!(err_odd.is_err());
    assert!(err_odd.unwrap_err().contains("Invalid receiver geometry"));
}
