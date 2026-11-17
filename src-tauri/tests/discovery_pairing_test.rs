//! Unit tests for LAN Auto-Discovery (mDNS) & PIN Pairing Security
//!
//! Tests PeerDevice serialization, mDNS lifecycle (simulated), PIN generation,
//! verification, wrong PIN rejection, 3-attempt lockout, and session expiration.

use std::net::{IpAddr, Ipv4Addr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use wireless_monitor_lib::discovery::{
    DiscoveryService, MdnsDiscovery, PeerDevice, SERVICE_TYPE,
};
use wireless_monitor_lib::pairing::{
    PairingSession, PairingState, PinAuthError, PinAuthManager,
};

#[test]
fn test_peer_device_serialization_and_deserialization() {
    let peer = PeerDevice {
        id: "peer-host-001".to_string(),
        name: "Host-Workstation".to_string(),
        ip: IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)),
        port: 8080,
        mode: "host".to_string(),
    };

    assert!(peer.is_host());
    assert!(!peer.is_monitor());

    let json = serde_json::to_string(&peer).expect("Serialization failed");
    assert!(json.contains("peer-host-001"));
    assert!(json.contains("192.168.1.100"));
    assert!(json.contains("8080"));
    assert!(json.contains("host"));

    let deserialized: PeerDevice =
        serde_json::from_str(&json).expect("Deserialization failed");
    assert_eq!(deserialized.id, peer.id);
    assert_eq!(deserialized.name, peer.name);
    assert_eq!(deserialized.ip, peer.ip);
    assert_eq!(deserialized.port, peer.port);
    assert_eq!(deserialized.mode, peer.mode);
}

#[test]
fn test_peer_device_invalid_json_rejection() {
    let invalid_json = r#"{"id": "test", "name": "bad"}"#; // missing ip, port, mode
    let res = serde_json::from_str::<PeerDevice>(invalid_json);
    assert!(res.is_err());
}

#[test]
fn test_pin_generation_format_and_range() {
    let mut manager = PinAuthManager::new();

    for i in 0..20 {
        let peer_id = format!("peer-{}", i);
        let pin = manager.generate_pin(&peer_id);

        assert_eq!(pin.len(), 4, "PIN must be exactly 4 digits");
        assert!(
            pin.chars().all(|c| c.is_ascii_digit()),
            "PIN must only contain numeric digits: {}",
            pin
        );

        let pin_val: u32 = pin.parse().expect("Failed to parse PIN as integer");
        assert!(
            (1000..=9999).contains(&pin_val),
            "PIN value {} must be in range 1000..=9999",
            pin_val
        );

        let session = manager.get_session(&peer_id).expect("Session must exist");
        assert_eq!(session.pin, pin);
        assert_eq!(session.attempts, 0);
        assert_eq!(session.state, PairingState::Pending);
        assert_eq!(manager.remaining_attempts(&peer_id), Some(3));
        assert!(!manager.is_locked_out(&peer_id));
        assert!(!manager.is_expired(&peer_id));
    }
}

#[test]
fn test_pin_verification_success() {
    let mut manager = PinAuthManager::new();
    let peer_id = "test-receiver-1";
    let pin = manager.generate_pin(peer_id);

    let result = manager.verify_pin(peer_id, &pin);
    assert!(result.is_ok());
    assert!(result.unwrap(), "Correct PIN verification must return true");

    let session = manager.get_session(peer_id).expect("Session must exist");
    assert_eq!(session.state, PairingState::Authenticated);
}

#[test]
fn test_pin_verification_wrong_pin_rejection() {
    let mut manager = PinAuthManager::new();
    let peer_id = "test-receiver-wrong";
    let pin = manager.generate_pin(peer_id);

    // Pick a wrong PIN
    let wrong_pin = if pin == "1234" { "4321" } else { "1234" };

    let result = manager.verify_pin(peer_id, wrong_pin);
    assert!(result.is_ok());
    assert!(
        !result.unwrap(),
        "Mismatched PIN verification must return false"
    );

    let session = manager.get_session(peer_id).expect("Session must exist");
    assert_eq!(session.attempts, 1);
    assert_eq!(session.remaining_attempts(), 2);
    assert_eq!(manager.remaining_attempts(peer_id), Some(2));
    assert_eq!(session.state, PairingState::Pending);
}

#[test]
fn test_pin_verification_invalid_format_rejection() {
    let mut manager = PinAuthManager::new();
    let peer_id = "test-receiver-format";
    manager.generate_pin(peer_id);

    // Test invalid lengths and non-digits
    assert!(manager.verify_pin(peer_id, "123").is_err());
    assert!(manager.verify_pin(peer_id, "12345").is_err());
    assert!(manager.verify_pin(peer_id, "abcd").is_err());
    assert!(manager.verify_pin(peer_id, "12a4").is_err());
    assert!(manager.verify_pin(peer_id, "").is_err());
}

#[test]
fn test_pin_lockout_after_three_failed_attempts() {
    let mut manager = PinAuthManager::new();
    let peer_id = "test-receiver-lockout";
    let correct_pin = manager.generate_pin(peer_id);
    let wrong_pin = if correct_pin == "9999" { "8888" } else { "9999" };

    // Attempt 1: wrong PIN
    let res1 = manager.verify_pin(peer_id, wrong_pin);
    assert_eq!(res1.unwrap(), false);
    assert_eq!(manager.remaining_attempts(peer_id), Some(2));
    assert!(!manager.is_locked_out(peer_id));

    // Attempt 2: wrong PIN
    let res2 = manager.verify_pin(peer_id, wrong_pin);
    assert_eq!(res2.unwrap(), false);
    assert_eq!(manager.remaining_attempts(peer_id), Some(1));
    assert!(!manager.is_locked_out(peer_id));

    // Attempt 3: wrong PIN -> locks out session
    let res3 = manager.verify_pin(peer_id, wrong_pin);
    assert_eq!(res3.unwrap(), false);
    assert_eq!(manager.remaining_attempts(peer_id), Some(0));
    assert!(manager.is_locked_out(peer_id));

    let session = manager.get_session(peer_id).expect("Session must exist");
    assert_eq!(session.state, PairingState::LockedOut);

    // Attempt 4: Even with the CORRECT PIN, verification must fail with Lockout error
    let res4 = manager.verify_pin(peer_id, &correct_pin);
    assert!(res4.is_err(), "Verification after lockout must return Err");
    let err_msg = res4.unwrap_err().to_string();
    assert!(
        err_msg.contains("locked out") || err_msg.contains("exceeded"),
        "Error message should mention lockout: {}",
        err_msg
    );
}

#[test]
fn test_pin_session_expiration_handling() {
    let mut manager = PinAuthManager::new();
    let peer_id = "test-receiver-expired";
    let correct_pin = manager.generate_pin(peer_id);

    // Manually expire session for test
    manager.expire_session_for_test(peer_id);
    assert!(manager.is_expired(peer_id));

    let res = manager.verify_pin(peer_id, &correct_pin);
    assert!(res.is_err(), "Verification on expired session must return Err");
    let err_msg = res.unwrap_err().to_string();
    assert!(
        err_msg.contains("expired") || err_msg.contains("Expired"),
        "Error should indicate expiration: {}",
        err_msg
    );

    let session = manager.get_session(peer_id).expect("Session must exist");
    assert_eq!(session.state, PairingState::Expired);
}

#[test]
fn test_pin_unknown_session_rejection() {
    let mut manager = PinAuthManager::new();
    let res = manager.verify_pin("nonexistent-peer", "1234");
    assert!(res.is_err(), "Verifying nonexistent session must return Err");
}

#[test]
fn test_pin_clear_session() {
    let mut manager = PinAuthManager::new();
    let peer_id = "test-clear-peer";
    manager.generate_pin(peer_id);
    assert!(manager.get_session(peer_id).is_some());

    manager.clear_session(peer_id);
    assert!(manager.get_session(peer_id).is_none());
    assert!(manager.verify_pin(peer_id, "1234").is_err());
}

#[test]
fn test_pin_regenerate_resets_lockout_and_session() {
    let mut manager = PinAuthManager::new();
    let peer_id = "test-reset-lockout";
    let pin1 = manager.generate_pin(peer_id);
    let wrong_pin = if pin1 == "1111" { "2222" } else { "1111" };

    // Exhaust attempts
    for _ in 0..3 {
        let _ = manager.verify_pin(peer_id, wrong_pin);
    }
    assert!(manager.is_locked_out(peer_id));

    // Regenerate new PIN
    let pin2 = manager.generate_pin(peer_id);
    assert!(!manager.is_locked_out(peer_id));
    assert_eq!(manager.remaining_attempts(peer_id), Some(3));

    // Verify with new PIN
    let res = manager.verify_pin(peer_id, &pin2);
    assert!(res.is_ok());
    assert!(res.unwrap());
}

#[test]
fn test_mdns_discovery_simulated_lifecycle() {
    let discovery = MdnsDiscovery::new_simulated();
    assert!(discovery.is_simulated());
    assert!(!discovery.is_advertising());
    assert!(!discovery.is_browsing());

    // Start advertising
    let res = discovery.start_advertising("Host-Test-PC", 8080, "host");
    assert!(res.is_ok());
    assert!(discovery.is_advertising());

    let advertised = discovery.get_advertised_info();
    assert!(advertised.is_some());
    let (name, port, mode) = advertised.unwrap();
    assert_eq!(name, "Host-Test-PC");
    assert_eq!(port, 8080);
    assert_eq!(mode, "host");

    // Stop service
    let stop_res = discovery.stop();
    assert!(stop_res.is_ok());
    assert!(!discovery.is_advertising());
}

#[test]
fn test_mdns_discovery_browsing_and_peer_detection() {
    let discovery = MdnsDiscovery::new_simulated();
    let peer_found_flag = Arc::new(AtomicBool::new(false));
    let flag_clone = Arc::clone(&peer_found_flag);

    let browse_res = discovery.start_browsing(move |peer: PeerDevice| {
        if peer.id == "monitor-device-42" {
            flag_clone.store(true, Ordering::SeqCst);
        }
    });
    assert!(browse_res.is_ok());
    assert!(discovery.is_browsing());

    // Inject simulated discovered peer
    let peer = PeerDevice {
        id: "monitor-device-42".to_string(),
        name: "LivingRoom-Tablet".to_string(),
        ip: IpAddr::V4(Ipv4Addr::new(192, 168, 1, 150)),
        port: 8081,
        mode: "monitor".to_string(),
    };

    let sim_res = discovery.simulate_peer_discovered(peer.clone());
    assert!(sim_res.is_ok());
    assert!(peer_found_flag.load(Ordering::SeqCst));

    let peers = discovery.get_discovered_peers();
    assert_eq!(peers.len(), 1);
    assert_eq!(peers[0].id, "monitor-device-42");
    assert_eq!(peers[0].port, 8081);

    // Stop browsing
    assert!(discovery.stop().is_ok());
    assert!(!discovery.is_browsing());
}

#[test]
fn test_mdns_discovery_thread_safety_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<MdnsDiscovery>();
    assert_send_sync::<PeerDevice>();
    assert_send_sync::<PairingSession>();
    assert_send_sync::<PairingState>();
    assert_send_sync::<PinAuthError>();
}
