//! End-to-End Orchestration Integration Tests
//!
//! Verifies complete application lifecycle:
//! 1. Host and Monitor discovery over mDNS
//! 2. Pairing challenge and PIN verification
//! 3. Virtual display creation via WindowsIddCxManager
//! 4. Screen capture and frame compositing via DxgiCapturer
//! 5. H.264 encoding and WebRTC RTP streaming
//! 6. Receiver depacketization and presentation surface rendering
//! 7. RTCP feedback loop (PLI/FIR)
//! 8. Session termination and clean teardown safety (destroy_all on shutdown)

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex as TokioMutex;

use wireless_monitor_lib::capture::{CapturedFrame, DxgiCapturer, ScreenCapturer};
use wireless_monitor_lib::discovery::{DiscoveryService, MdnsDiscovery, PeerDevice};
use wireless_monitor_lib::display::{DisplayConfig, VirtualDisplayManager, WindowsIddCxManager};
use wireless_monitor_lib::pairing::{PairingState, PinAuthManager};
use wireless_monitor_lib::receiver::{
    ClientScreenGeometry, MonitorReceiver, ReceiverConfig, SurfaceMode,
};
use wireless_monitor_lib::stream::{
    create_mock_encoder, EncodedPacket, MockVideoEncoder, RtpPacket, RtpPacketizer, SessionState,
    StreamConfig, VideoEncoder, WebRtcHostSession,
};
use wireless_monitor_lib::{
    AppState, DeviceInfo, SessionStatus, SystemStatus,
};

#[tokio::test]
async fn test_system_status_and_device_info() {
    let app_state = AppState::new_simulated();

    let status = app_state.check_system_status();
    assert!(!status.os.is_empty());
    assert!(!status.hostname.is_empty());
    assert!(status.is_host_ready);
    assert!(status.is_monitor_ready);
    assert!(status.driver_installed);

    let dev_info = app_state.get_device_info();
    assert_eq!(dev_info.hostname, status.hostname);
    assert_eq!(dev_info.os, status.os);
    assert!(dev_info.driver_installed);
    assert!(dev_info.active_session.is_none());
    assert_eq!(dev_info.port, 5000);
}

#[tokio::test]
async fn test_discovery_start_and_peer_tracking() {
    let app_state = AppState::new_simulated();

    // Start discovery
    let res = app_state.start_discovery();
    assert!(res.is_ok(), "Discovery should start successfully");

    // Initially no peers
    let initial_peers = app_state.get_discovered_peers();
    assert_eq!(initial_peers.len(), 0);

    // Simulate discovering two peer devices on the network
    let peer_monitor = PeerDevice::new(
        "peer-mon-1",
        "Living Room Laptop",
        "192.168.1.100".parse().unwrap(),
        5000,
        "monitor",
    );
    let peer_host = PeerDevice::new(
        "peer-host-2",
        "Office Desktop",
        "192.168.1.101".parse().unwrap(),
        5000,
        "host",
    );

    app_state.discovery_service().inject_peer(peer_monitor.clone());
    app_state.discovery_service().inject_peer(peer_host.clone());

    let peers = app_state.get_discovered_peers();
    assert_eq!(peers.len(), 2);
    assert!(peers.iter().any(|p| p.id == "peer-mon-1" && p.is_monitor()));
    assert!(peers.iter().any(|p| p.id == "peer-host-2" && p.is_host()));
}

#[tokio::test]
async fn test_pairing_challenge_and_pin_submission() {
    let app_state = AppState::new_simulated();
    let peer_id = "test-monitor-42";

    // Inject peer device
    app_state.discovery_service().inject_peer(PeerDevice::new(
        peer_id,
        "Test Tablet",
        "192.168.1.150".parse().unwrap(),
        5000,
        "monitor",
    ));

    // Initiate extend display session
    let session = app_state
        .initiate_extend_display(peer_id)
        .expect("Initiate should succeed");
    assert_eq!(session.peer_id, peer_id);
    assert_eq!(session.state, "pairing");
    assert!(session.monitor_id.is_none());

    // Submit invalid PIN: length != 4
    let bad_format = app_state.submit_pin(peer_id, "12");
    assert!(bad_format.is_err(), "Invalid PIN format must fail");

    // Submit wrong 4-digit PIN
    let wrong_pin = app_state.submit_pin(peer_id, "0000");
    assert_eq!(wrong_pin, Ok(false), "Incorrect PIN returns Ok(false)");

    // Retrieve active challenge PIN from auth manager
    let correct_pin = {
        let auth = app_state.pin_auth_manager.lock().unwrap();
        auth.get_session(peer_id)
            .expect("Session exists")
            .pin
            .clone()
    };

    // Submit correct PIN
    let pair_ok = app_state
        .submit_pin(peer_id, &correct_pin)
        .expect("Submit pin call");
    assert!(pair_ok, "Pairing should succeed with correct PIN");

    // Session should now be active
    let active_session = app_state.get_active_session().expect("Active session exists");
    assert_eq!(active_session.state, "active");
    assert!(active_session.monitor_id.is_some());
    assert_eq!(active_session.resolution.as_deref(), Some("1920x1080"));
    assert_eq!(active_session.fps, Some(60));

    // Virtual display should be created
    let display_count = {
        let disp = app_state.display_manager.lock().unwrap();
        disp.display_count()
    };
    assert_eq!(display_count, 1, "One virtual display must be active");
}

#[tokio::test]
async fn test_pin_lockout_exceeding_max_attempts() {
    let app_state = AppState::new_simulated();
    let peer_id = "attacker-device";

    let _session = app_state
        .initiate_extend_display(peer_id)
        .expect("Initiate session");

    // Submit wrong PIN 3 times
    assert_eq!(app_state.submit_pin(peer_id, "0001"), Ok(false));
    assert_eq!(app_state.submit_pin(peer_id, "0002"), Ok(false));
    let third_attempt = app_state.submit_pin(peer_id, "0003");
    assert!(
        third_attempt.is_err(),
        "3rd failed attempt must lock out session"
    );

    // 4th attempt should also fail with lockout
    let fourth_attempt = app_state.submit_pin(peer_id, "0004");
    assert!(
        fourth_attempt.is_err(),
        "Subsequent attempt must remain locked out"
    );
}

#[tokio::test]
async fn test_end_to_end_frame_capture_to_receiver_presentation() {
    let app_state = AppState::new_simulated();
    let peer_id = "e2e-mon-receiver";

    // 1. Discovery & Peer setup
    app_state.discovery_service().inject_peer(PeerDevice::new(
        peer_id,
        "E2E Receiver",
        "127.0.0.1".parse().unwrap(),
        5000,
        "monitor",
    ));

    // 2. Initiate & Pair
    let _ = app_state.initiate_extend_display(peer_id).unwrap();
    let pin = {
        let auth = app_state.pin_auth_manager.lock().unwrap();
        auth.get_session(peer_id).unwrap().pin.clone()
    };
    assert!(app_state.submit_pin(peer_id, &pin).unwrap());

    // 3. Verify Virtual Display was created
    let active_monitor_id = {
        let disp = app_state.display_manager.lock().unwrap();
        assert_eq!(disp.display_count(), 1);
        disp.active_displays()[0]
    };

    // 4. Start Capture on the active display
    {
        let mut capt = app_state.screen_capturer.lock().unwrap();
        if let Some(c) = capt.as_mut() {
            c.start(active_monitor_id).expect("Capturer start");
        }
    }

    // Capture a simulated desktop frame
    let frame = {
        let mut capt = app_state.screen_capturer.lock().unwrap();
        let capturer = capt.as_mut().expect("Capturer exists");
        capturer
            .capture_frame(100)
            .expect("Capture frame")
            .expect("Frame available")
    };
    assert_eq!(frame.width, 1920);
    assert_eq!(frame.height, 1080);
    assert!(frame.is_valid());

    // 5. Initialize WebRTC Host Session and Ingest Frame
    let stream_cfg = StreamConfig::default();
    let host_session = WebRtcHostSession::init(stream_cfg.clone()).await.expect("Init host");
    let mut host_guard = app_state.host_session.lock().unwrap();
    *host_guard = Some(host_session);

    let host = host_guard.as_mut().unwrap();
    let _offer = host.create_offer().await.expect("Create offer");
    host.apply_answer("v=0\r\no=- 2 2 IN IP4 127.0.0.1\r\ns=-\r\nt=0 0\r\nm=video 5000 RTP/AVP 96\r\n")
        .await
        .expect("Apply answer");

    host.ingest_frame(frame).await.expect("Ingest frame");

    let packets = host.take_pending_packets();
    assert!(!packets.is_empty(), "RTP packets must be generated from frame");

    // 6. Connect MonitorReceiver and Ingest RTP Packets
    let geometry = ClientScreenGeometry::new(1920, 1080, 60).unwrap();
    let mut receiver = MonitorReceiver::connect("127.0.0.1", 5000, "1234", geometry)
        .await
        .expect("Connect receiver");
    receiver
        .start_presentation(ReceiverConfig::default())
        .await
        .expect("Start presentation");

    let mut reassembled_frame: Option<CapturedFrame> = None;
    for pkt in &packets {
        if let Some(f) = receiver.reassemble_rtp_packet(pkt).expect("Reassemble") {
            reassembled_frame = Some(f);
        }
    }

    assert!(
        reassembled_frame.is_some(),
        "Receiver must reassemble frame from packets"
    );

    // Queue frame into receiver presentation surface and render
    let rf = reassembled_frame.unwrap();
    receiver.surface_mut().queue_frame(rf).expect("Queue frame");
    let rendered = receiver.surface_mut().render_frame().expect("Render frame");
    assert!(rendered.is_some(), "Frame must be presented on surface");

    let surface_stats = receiver.surface().stats();
    assert_eq!(surface_stats.rendered_frames, 1);

    // 7. Verify RTCP PLI Loop
    // Simulate packet loss by jumping sequence numbers
    let gap_pkt = RtpPacket {
        payload_type: 96,
        sequence_number: packets.last().unwrap().sequence_number.wrapping_add(5),
        timestamp: 90000,
        ssrc: 12345,
        marker: true,
        payload: vec![0x1C, 0x41, 0x00],
    };
    let _ = receiver.reassemble_rtp_packet(&gap_pkt);
    let rtcp_feedback = receiver.take_pending_rtcp();
    assert!(
        !rtcp_feedback.is_empty(),
        "Sequence gap must trigger RTCP PLI feedback"
    );

    // Host processes RTCP feedback and triggers keyframe
    host.handle_rtcp_feedback(&rtcp_feedback[0])
        .await
        .expect("Handle feedback");
    let host_stats = host.stats();
    assert_eq!(
        host_stats.keyframe_requests_received, 1,
        "Host must register keyframe request from receiver PLI"
    );
}

#[tokio::test]
async fn test_clean_session_stop_and_teardown() {
    let app_state = AppState::new_simulated();
    let peer_id = "test-stop-peer";

    // Set up active session
    let _ = app_state.initiate_extend_display(peer_id).unwrap();
    let pin = {
        let auth = app_state.pin_auth_manager.lock().unwrap();
        auth.get_session(peer_id).unwrap().pin.clone()
    };
    assert!(app_state.submit_pin(peer_id, &pin).unwrap());

    // Verify session is active with 1 display
    assert!(app_state.get_active_session().is_some());
    assert_eq!(app_state.display_manager.lock().unwrap().display_count(), 1);

    // Stop session
    let stop_res = app_state.stop_active_session().await;
    assert!(stop_res.is_ok(), "Session stop should succeed");

    // Verify displays destroyed and session cleared
    assert_eq!(
        app_state.display_manager.lock().unwrap().display_count(),
        0,
        "All virtual displays must be destroyed on session stop"
    );
    assert!(
        app_state.get_active_session().is_none(),
        "Active session must be None after stop"
    );

    // Stopping again when no session is active should be a safe no-op
    assert!(app_state.stop_active_session().await.is_ok());
}

#[tokio::test]
async fn test_shutdown_hook_orphan_prevention() {
    let app_state = AppState::new_simulated();
    let peer_id = "shutdown-test-peer";

    // Set up active session with virtual display
    let _ = app_state.initiate_extend_display(peer_id).unwrap();
    let pin = {
        let auth = app_state.pin_auth_manager.lock().unwrap();
        auth.get_session(peer_id).unwrap().pin.clone()
    };
    assert!(app_state.submit_pin(peer_id, &pin).unwrap());
    assert_eq!(app_state.display_manager.lock().unwrap().display_count(), 1);

    // Execute teardown hook (simulates window close / process termination)
    app_state.teardown();

    // Verify all virtual displays are cleanly destroyed
    assert_eq!(
        app_state.display_manager.lock().unwrap().display_count(),
        0,
        "Virtual display must not be orphaned after teardown"
    );
}

#[tokio::test]
async fn test_concurrent_and_invalid_commands() {
    let app_state = AppState::new_simulated();

    // Initiate extend display with empty peer_id should fail
    let res = app_state.initiate_extend_display("");
    assert!(res.is_err(), "Empty peer ID must be rejected");

    // Submit PIN for unknown peer should fail
    let pin_res = app_state.submit_pin("unknown-peer", "1234");
    assert!(pin_res.is_err(), "Unknown peer PIN submit must fail");

    // Stop session when none exists should succeed cleanly
    let stop_res = app_state.stop_active_session().await;
    assert!(stop_res.is_ok());
}

#[test]
fn test_app_state_thread_safety_send_sync() {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    assert_send::<AppState>();
    assert_sync::<AppState>();
    assert_send::<SystemStatus>();
    assert_sync::<SystemStatus>();
    assert_send::<DeviceInfo>();
    assert_sync::<DeviceInfo>();
    assert_send::<SessionStatus>();
    assert_sync::<SessionStatus>();
}
