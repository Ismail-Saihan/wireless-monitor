//! Unit & integration tests for Connection Approval Workflow & Enhanced PIN Security (Phase 3 Task 2)
//!
//! Tests:
//! 1. Incoming connection request generation and queue tracking
//! 2. Receiver approval state transition: PendingApproval -> PendingPin -> Authenticated
//! 3. Receiver rejection state transition: PendingApproval -> Rejected (PIN blocked)
//! 4. Prevention of PIN verification while still in PendingApproval
//! 5. 3-attempt lockout enforcement and remaining attempts counter
//! 6. 300-second session expiration enforcement
//! 7. Concurrent incoming request queue management (FIFO / timestamp ordering)
//! 8. Rejection of unknown peer sessions
//! 9. Tauri AppState integration ensuring zero orphaned displays on rejection or lockout
//! 10. Serde serialization of IncomingRequest and PairingState

use std::sync::Arc;
use std::time::Duration;

use wireless_monitor_lib::pairing::{
    IncomingRequest, PairingSession, PairingState, PinAuthError, PinAuthManager,
};
use wireless_monitor_lib::AppState;

#[test]
fn test_incoming_request_generation_and_queue() {
    let mut manager = PinAuthManager::new();
    let peer_id = "receiver-macbook-1";
    let peer_name = "MacBook Pro 16";

    let (pin, session) = manager.request_connection(peer_id, peer_name);

    // Verify generated PIN properties
    assert_eq!(pin.len(), 4, "PIN must be exactly 4 numeric digits");
    assert!(
        pin.chars().all(|c| c.is_ascii_digit()),
        "PIN must contain only digits: {}",
        pin
    );
    let pin_num: u32 = pin.parse().expect("Failed to parse PIN");
    assert!((1000..=9999).contains(&pin_num), "PIN must be in range 1000..=9999");

    // Verify session state is initially PendingApproval
    assert_eq!(session.peer_id, peer_id);
    assert_eq!(session.pin, pin);
    assert_eq!(session.state, PairingState::PendingApproval);
    assert_eq!(session.attempts, 0);
    assert_eq!(session.max_attempts, 3);
    assert_eq!(session.remaining_attempts(), 3);

    // Verify incoming request can be fetched individually
    let req = manager
        .get_pending_request(peer_id)
        .expect("Pending request must exist for peer");
    assert_eq!(req.peer_id, peer_id);
    assert_eq!(req.peer_name, peer_name);
    assert_eq!(req.pin, pin);
    assert!(req.requested_at > 0, "Timestamp must be non-zero");

    // Verify request is present in get_all_pending_requests
    let all = manager.get_all_pending_requests();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].peer_id, peer_id);
}

#[test]
fn test_connection_approval_transition_to_pending_pin() {
    let mut manager = PinAuthManager::new();
    let peer_id = "peer-desktop-approval";
    let peer_name = "Workstation Desktop";

    let (pin, _) = manager.request_connection(peer_id, peer_name);

    // Verify session starts in PendingApproval
    let session_before = manager.get_session(peer_id).expect("Session must exist");
    assert_eq!(session_before.state, PairingState::PendingApproval);

    // Approve the connection
    let res = manager.respond_connection(peer_id, true);
    assert!(res.is_ok(), "Approval must succeed");
    assert!(res.unwrap(), "Approval result must be true");

    // State must now be PendingPin
    let session_after = manager.get_session(peer_id).expect("Session must exist");
    assert_eq!(session_after.state, PairingState::PendingPin);

    // Request must no longer be present in the pending requests queue
    assert!(manager.get_pending_request(peer_id).is_none());
    assert!(manager.get_all_pending_requests().is_empty());

    // PIN verification can now succeed
    let verify_res = manager.verify_pin(peer_id, &pin);
    assert!(verify_res.is_ok());
    assert!(verify_res.unwrap(), "Correct PIN must authenticate");

    let session_final = manager.get_session(peer_id).expect("Session must exist");
    assert_eq!(session_final.state, PairingState::Authenticated);
}

#[test]
fn test_connection_rejection_transition_to_rejected() {
    let mut manager = PinAuthManager::new();
    let peer_id = "peer-unwanted-device";
    let peer_name = "Rogue Device";

    let (pin, _) = manager.request_connection(peer_id, peer_name);

    // Reject the connection
    let res = manager.respond_connection(peer_id, false);
    assert!(res.is_ok(), "Rejection must return Ok(false)");
    assert!(!res.unwrap(), "Rejection result must be false");

    // Session must be in Rejected state
    let session = manager.get_session(peer_id).expect("Session must exist");
    assert_eq!(session.state, PairingState::Rejected);

    // Request must be removed from queue
    assert!(manager.get_pending_request(peer_id).is_none());
    assert!(manager.get_all_pending_requests().is_empty());

    // Attempting to verify PIN on rejected session must fail
    let verify_res = manager.verify_pin(peer_id, &pin);
    assert!(verify_res.is_err(), "Rejected session must reject PIN verification");

    // Attempting to approve after rejection must fail
    let re_approval = manager.respond_connection(peer_id, true);
    assert!(re_approval.is_err(), "Cannot approve an already rejected session");
}

#[test]
fn test_verify_pin_fails_while_pending_approval() {
    let mut manager = PinAuthManager::new();
    let peer_id = "peer-pending-auth";
    let peer_name = "Living Room TV";

    let (pin, _) = manager.request_connection(peer_id, peer_name);

    // Attempt to verify PIN while still in PendingApproval
    let verify_res = manager.verify_pin(peer_id, &pin);
    assert!(
        verify_res.is_err(),
        "Must not allow PIN verification while approval is pending"
    );

    // Session must remain in PendingApproval
    let session = manager.get_session(peer_id).expect("Session must exist");
    assert_eq!(session.state, PairingState::PendingApproval);
}

#[test]
fn test_three_attempt_lockout_enforcement() {
    let mut manager = PinAuthManager::new();
    let peer_id = "peer-lockout-test";
    let peer_name = "Laptop";

    let (pin, _) = manager.request_connection(peer_id, peer_name);
    assert!(manager.respond_connection(peer_id, true).unwrap());

    let wrong_pin = if pin == "1234" { "4321" } else { "1234" };

    // Attempt 1: wrong PIN
    let res1 = manager.verify_pin(peer_id, wrong_pin).expect("Attempt 1 failed");
    assert!(!res1);
    assert_eq!(manager.remaining_attempts(peer_id), Some(2));
    assert!(!manager.is_locked_out(peer_id));

    // Attempt 2: wrong PIN
    let res2 = manager.verify_pin(peer_id, wrong_pin).expect("Attempt 2 failed");
    assert!(!res2);
    assert_eq!(manager.remaining_attempts(peer_id), Some(1));
    assert!(!manager.is_locked_out(peer_id));

    // Attempt 3: wrong PIN -> triggers lockout
    let res3 = manager.verify_pin(peer_id, wrong_pin).expect("Attempt 3 failed");
    assert!(!res3);
    assert_eq!(manager.remaining_attempts(peer_id), Some(0));
    assert!(manager.is_locked_out(peer_id));

    let session = manager.get_session(peer_id).expect("Session must exist");
    assert_eq!(session.state, PairingState::LockedOut);

    // Attempt 4: even with correct PIN, must bail with MaxAttemptsExceeded
    let res4 = manager.verify_pin(peer_id, &pin);
    assert!(res4.is_err(), "Session must be locked out on attempt 4");
}

#[test]
fn test_session_expiration_enforcement_300s() {
    let mut manager = PinAuthManager::with_ttl(Duration::from_millis(50));
    let peer_id = "peer-expiration-test";
    let peer_name = "Tablet";

    let (pin, _) = manager.request_connection(peer_id, peer_name);

    // Initially active
    assert!(!manager.is_expired(peer_id));
    assert!(manager.get_pending_request(peer_id).is_some());

    // Sleep past TTL
    std::thread::sleep(Duration::from_millis(70));

    // Must be expired
    assert!(manager.is_expired(peer_id));

    // Expired requests must not appear in pending list
    assert!(manager.get_all_pending_requests().is_empty());

    // Verification must fail with Expired
    let res = manager.verify_pin(peer_id, &pin);
    assert!(res.is_err());

    // Response must fail with Expired
    let resp = manager.respond_connection(peer_id, true);
    assert!(resp.is_err());
}

#[test]
fn test_multiple_concurrent_incoming_requests() {
    let mut manager = PinAuthManager::new();

    let (pin1, _) = manager.request_connection("peer-1", "Client One");
    let (_, _) = manager.request_connection("peer-2", "Client Two");
    let (_, _) = manager.request_connection("peer-3", "Client Three");

    let pending = manager.get_all_pending_requests();
    assert_eq!(pending.len(), 3);
    assert_eq!(pending[0].peer_id, "peer-1");
    assert_eq!(pending[1].peer_id, "peer-2");
    assert_eq!(pending[2].peer_id, "peer-3");

    // Approve peer-1
    assert!(manager.respond_connection("peer-1", true).unwrap());
    // Reject peer-2
    assert!(!manager.respond_connection("peer-2", false).unwrap());

    // Only peer-3 remains in queue
    let remaining = manager.get_all_pending_requests();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].peer_id, "peer-3");

    // Verify peer-1 can authenticate
    assert!(manager.verify_pin("peer-1", &pin1).unwrap());
    assert_eq!(
        manager.get_session("peer-1").unwrap().state,
        PairingState::Authenticated
    );

    // Verify peer-2 cannot authenticate
    assert_eq!(
        manager.get_session("peer-2").unwrap().state,
        PairingState::Rejected
    );
}

#[test]
fn test_respond_nonexistent_session() {
    let mut manager = PinAuthManager::new();
    let res = manager.respond_connection("ghost-peer", true);
    assert!(res.is_err());
    match res {
        Err(PinAuthError::SessionNotFound(peer)) => assert_eq!(peer, "ghost-peer"),
        other => panic!("Expected SessionNotFound, got: {:?}", other),
    }
}

#[test]
fn test_orphan_display_prevention_on_rejected_or_locked_out_session() {
    let app_state = AppState::new_simulated();

    // 1. Unapproved / rejected session
    {
        let mut auth = app_state.pin_auth_manager.lock().unwrap();
        auth.request_connection("peer-rejected-orphan", "Spy Client");
        auth.respond_connection("peer-rejected-orphan", false).unwrap();
    }

    // Try to submit PIN through AppState
    let res = app_state.submit_pin("peer-rejected-orphan", "1234");
    assert!(res.is_err(), "Rejected session must fail submit_pin");

    // Ensure no display was created and active_session is None
    assert!(
        app_state.get_active_session().is_none(),
        "Rejected session must never spawn an active display session"
    );

    // 2. Locked out session
    let valid_pin = {
        let mut auth = app_state.pin_auth_manager.lock().unwrap();
        let (pin, _) = auth.request_connection("peer-locked-orphan", "Bad Typist");
        auth.respond_connection("peer-locked-orphan", true).unwrap();
        pin
    };

    // Fail 3 times
    let _ = app_state.submit_pin("peer-locked-orphan", "0000");
    let _ = app_state.submit_pin("peer-locked-orphan", "0000");
    let _ = app_state.submit_pin("peer-locked-orphan", "0000");

    // Submit correct pin while locked out
    let locked_res = app_state.submit_pin("peer-locked-orphan", &valid_pin);
    assert!(
        locked_res.is_err(),
        "Locked out session must fail submit_pin"
    );
    assert!(
        app_state.get_active_session().is_none(),
        "Locked out session must never spawn an active display session"
    );
}

#[test]
fn test_incoming_request_and_state_serialization() {
    let req = IncomingRequest {
        peer_id: "peer-json-1".to_string(),
        peer_name: "Android Tablet".to_string(),
        pin: "5678".to_string(),
        requested_at: 1700000000,
    };

    let json = serde_json::to_string(&req).expect("Failed to serialize IncomingRequest");
    assert!(json.contains("peer-json-1"));
    assert!(json.contains("Android Tablet"));
    assert!(json.contains("5678"));

    let deserialized: IncomingRequest =
        serde_json::from_str(&json).expect("Failed to deserialize IncomingRequest");
    assert_eq!(deserialized, req);

    let states = vec![
        PairingState::PendingApproval,
        PairingState::PendingPin,
        PairingState::Authenticated,
        PairingState::LockedOut,
        PairingState::Expired,
        PairingState::Rejected,
    ];

    for state in states {
        let s_json = serde_json::to_string(&state).expect("Failed to serialize PairingState");
        let d_state: PairingState =
            serde_json::from_str(&s_json).expect("Failed to deserialize PairingState");
        assert_eq!(d_state, state);
    }
}
