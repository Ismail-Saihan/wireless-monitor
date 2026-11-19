//! Network signaling HTTP listener integration tests for Wireless Monitor

use std::time::Duration;
use wireless_monitor_lib::signaling::{
    connect_to_ip, run_signaling_server, send_get_info, send_request_connection, send_verify_pin,
    RequestConnectionPayload, VerifyPinPayload,
};
use wireless_monitor_lib::AppState;

fn find_available_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

#[tokio::test]
async fn test_signaling_http_info_endpoint() {
    let port = find_available_port();
    let state = AppState::new_simulated();
    let state_clone = state.clone();

    tokio::spawn(async move {
        let _ = run_signaling_server(state_clone, port).await;
    });

    // Wait briefly for server to bind
    tokio::time::sleep(Duration::from_millis(150)).await;

    let info = send_get_info("127.0.0.1", port)
        .await
        .expect("send_get_info must succeed");

    assert!(!info.device_id.is_empty());
    assert!(!info.hostname.is_empty());
    assert!(!info.os.is_empty());
    assert_eq!(info.role, "host");
    assert!(!info.capabilities.is_empty());
    assert_eq!(info.max_resolution, "1920x1080");
    assert_eq!(info.max_fps, 60);
}

#[tokio::test]
async fn test_signaling_http_request_connection_and_approval_queue() {
    let port = find_available_port();
    let state = AppState::new_simulated();
    let state_clone = state.clone();

    tokio::spawn(async move {
        let _ = run_signaling_server(state_clone, port).await;
    });

    tokio::time::sleep(Duration::from_millis(150)).await;

    let req = RequestConnectionPayload {
        peer_id: "client-laptop-1".to_string(),
        peer_name: "Ismail-Laptop".to_string(),
    };

    let resp = send_request_connection("127.0.0.1", port, &req)
        .await
        .expect("send_request_connection must succeed");

    assert_eq!(resp.pin.len(), 4);
    assert_eq!(resp.status, "pending_approval");

    // Verify incoming requests queue in receiver's PinAuthManager
    let incoming = state.get_incoming_requests();
    assert_eq!(incoming.len(), 1);
    assert_eq!(incoming[0].peer_id, "client-laptop-1");
    assert_eq!(incoming[0].peer_name, "Ismail-Laptop");
    assert_eq!(incoming[0].pin, resp.pin);
}

#[tokio::test]
async fn test_signaling_http_verify_pin_lifecycle() {
    let port = find_available_port();
    let state = AppState::new_simulated();
    let state_clone = state.clone();

    tokio::spawn(async move {
        let _ = run_signaling_server(state_clone, port).await;
    });

    tokio::time::sleep(Duration::from_millis(150)).await;

    let req = RequestConnectionPayload {
        peer_id: "client-laptop-2".to_string(),
        peer_name: "Second-Laptop".to_string(),
    };

    let resp = send_request_connection("127.0.0.1", port, &req)
        .await
        .expect("request-connection must succeed");

    let correct_pin = resp.pin;

    // Approve the request on the receiver side
    let approved = state.respond_to_request("client-laptop-2", true).unwrap();
    assert!(approved);

    // Verify wrong PIN -> false
    let wrong_payload = VerifyPinPayload {
        peer_id: "client-laptop-2".to_string(),
        pin: "0000".to_string(),
    };
    let wrong_resp = send_verify_pin("127.0.0.1", port, &wrong_payload)
        .await
        .expect("verify-pin call must return HTTP 200 response");
    assert!(!wrong_resp.success);

    // Verify correct PIN -> true
    let correct_payload = VerifyPinPayload {
        peer_id: "client-laptop-2".to_string(),
        pin: correct_pin,
    };
    let correct_resp = send_verify_pin("127.0.0.1", port, &correct_payload)
        .await
        .expect("verify-pin call must return HTTP 200 response");
    assert!(correct_resp.success);
}

#[tokio::test]
async fn test_connect_to_ip_discovery_injection() {
    let port = find_available_port();
    let server_state = AppState::new_simulated();
    let server_clone = server_state.clone();

    tokio::spawn(async move {
        let _ = run_signaling_server(server_clone, port).await;
    });

    tokio::time::sleep(Duration::from_millis(150)).await;

    let client_state = AppState::new_simulated();
    let ip_target = format!("127.0.0.1:{}", port);

    let peer = connect_to_ip(&client_state, &ip_target)
        .await
        .expect("connect_to_ip must connect and resolve peer");

    assert_eq!(peer.ip, "127.0.0.1".parse::<std::net::IpAddr>().unwrap());
    assert_eq!(peer.port, port);
    assert_eq!(peer.id, server_state.device_id);

    // Verify peer was injected into discovered peers
    let discovered = client_state.get_discovered_peers();
    let found = discovered.iter().any(|p| p.id == peer.id && p.port == port);
    assert!(found, "Injected peer must be present in discovery_service");
}
