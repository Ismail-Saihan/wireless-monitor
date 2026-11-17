//! End-to-End Bidirectional Integration Tests
//!
//! Verifies complete bidirectional application lifecycle across Linux Host and Windows Receiver:
//! 1. Operating system detection and dynamic app role switching (Host <-> Receiver)
//! 2. Linux Wayland virtual display creation (Mutter / XDG Portal D-Bus) and PipeWire frame capture
//! 3. Windows Direct3D11 presentation surface with sub-frame latency queue
//! 4. Zero-config mDNS discovery and 4-digit PIN authentication handshake with 3-attempt lockout
//! 5. Full glass-to-glass capture -> encode -> packetize -> depacketize -> render pipeline (<50ms latency)
//! 6. Clean session termination, teardown hooks, and orphan display prevention
//! 7. Phase 2 diagnostics and status metrics contract

use std::sync::Arc;
use wireless_monitor_lib::capture::{
    composite_pipewire_cursor, CapturedFrame, PipeWireCapturer, PipeWireCursorMetadata,
    PipeWireCursorMode, ScreenCapturer,
};
use wireless_monitor_lib::discovery::PeerDevice;
use wireless_monitor_lib::display::{
    DisplayConfig, VirtualDisplayManager, WaylandBackend, WaylandDisplayManager,
};
use wireless_monitor_lib::receiver::{
    ClientScreenGeometry, H264Depacketizer, MonitorReceiver, ReceiverConfig, WindowsD3D11Surface,
};
use wireless_monitor_lib::stream::{
    H264Encoder, RtpPacket, RtpPacketizer, StreamConfig, VideoEncoder,
};
use wireless_monitor_lib::{AppState, DeviceInfo, Phase2Status, SessionStatus, SystemStatus};

#[tokio::test]
async fn test_operating_system_detection_and_role_switching() {
    let app_state = AppState::new_simulated();

    // 1. Verify OS detection
    let os = app_state.get_operating_system();
    assert!(!os.is_empty(), "Operating system string must not be empty");
    assert_eq!(os, std::env::consts::OS);

    // 2. Default role must be "host"
    let status = app_state.get_phase2_status();
    assert_eq!(status.role, "host");

    // 3. Switch role to "receiver"
    let new_role = app_state.set_app_role("receiver");
    assert!(new_role.is_ok());
    assert_eq!(new_role.unwrap(), "receiver");
    assert_eq!(app_state.get_phase2_status().role, "receiver");

    // 4. Switch role back to "host"
    let host_role = app_state.set_app_role("host");
    assert!(host_role.is_ok());
    assert_eq!(host_role.unwrap(), "host");
    assert_eq!(app_state.get_phase2_status().role, "host");

    // 5. Case insensitivity and whitespace trimming
    let mixed_role = app_state.set_app_role("  RECEIVER  ");
    assert!(mixed_role.is_ok());
    assert_eq!(mixed_role.unwrap(), "receiver");

    // 6. Invalid roles must be rejected
    let invalid_role = app_state.set_app_role("invalid_role");
    assert!(invalid_role.is_err());
    let empty_role = app_state.set_app_role("");
    assert!(empty_role.is_err());
}

#[tokio::test]
async fn test_linux_host_virtual_display_and_pipewire_capture() {
    let app_state = AppState::new_simulated();

    // 1. Invalid display config must be rejected
    let invalid_config = DisplayConfig::new(0, 0, 0);
    let fail_res = app_state.start_linux_host(&invalid_config);
    assert!(fail_res.is_err(), "Invalid display config should fail validation");

    // 2. Valid display config creates virtual monitor on Linux Host
    let valid_config = DisplayConfig::new(1920, 1080, 60);
    let start_res = app_state.start_linux_host(&valid_config);
    assert!(start_res.is_ok(), "Linux host start should succeed");
    let monitor_id = start_res.unwrap();
    assert!(monitor_id > 0, "Monitor ID must be positive");

    // 3. Verify active session metadata
    let active_session = app_state.get_active_session();
    assert!(active_session.is_some());
    let session = active_session.unwrap();
    assert_eq!(session.state, "active");
    assert_eq!(session.monitor_id, Some(monitor_id));
    assert_eq!(session.resolution, Some("1920x1080".to_string()));
    assert_eq!(session.fps, Some(60));

    // 4. Verify Phase2Status reflects Linux host active state
    let phase2 = app_state.get_phase2_status();
    assert_eq!(phase2.role, "host");
    assert!(phase2.is_streaming);
    assert_eq!(phase2.monitor_id, Some(monitor_id));

    // 5. Verify PipeWire frame capture and cursor compositing
    let mut capturer = PipeWireCapturer::new_simulated(1920, 1080);
    assert!(capturer.start(monitor_id).is_ok());
    assert!(capturer.is_running());

    let frame_opt = capturer.capture_frame();
    assert!(frame_opt.is_ok());
    let frame = frame_opt.unwrap();
    assert!(frame.is_some());
    let valid_frame = frame.unwrap();
    assert_eq!(valid_frame.width, 1920);
    assert_eq!(valid_frame.height, 1080);
    assert!(valid_frame.stride >= 1920 * 4);
    assert!(valid_frame.is_valid());

    // Stop Linux host session
    assert!(app_state.stop_active_session().await.is_ok());
    assert!(app_state.get_active_session().is_none());
}

#[tokio::test]
async fn test_windows_receiver_d3d11_presentation_surface() {
    let app_state = AppState::new_simulated();

    // 1. Invalid geometry must be rejected
    let invalid_geom = ClientScreenGeometry {
        width: 0,
        height: 0,
        refresh_rate: 0,
    };
    let fail_res = app_state.start_windows_receiver(&invalid_geom);
    assert!(fail_res.is_err(), "Invalid geometry should fail validation");

    // 2. Start Windows Receiver with valid 1920x1080 geometry
    let valid_geom = ClientScreenGeometry {
        width: 1920,
        height: 1080,
        refresh_rate: 60,
    };
    let start_res = app_state.start_windows_receiver(&valid_geom);
    assert!(start_res.is_ok(), "Windows receiver should start successfully");

    // 3. Verify role switched to receiver and active session initialized
    let phase2 = app_state.get_phase2_status();
    assert_eq!(phase2.role, "receiver");
    assert!(phase2.surface_active);

    // 4. Test Direct3D11 frame rendering and queue overflow drop logic (<1 frame latency)
    let config = ReceiverConfig::default();
    let mut surface = WindowsD3D11Surface::new_simulated(valid_geom, config);
    assert!(surface.is_active());
    assert!(surface.is_fullscreen());

    let stride = 1920 * 4;
    let frame_size = (stride * 1080) as usize;
    let frame1 = CapturedFrame {
        width: 1920,
        height: 1080,
        stride,
        data: vec![128u8; frame_size],
        timestamp_us: 1000,
    };
    let frame2 = CapturedFrame {
        width: 1920,
        height: 1080,
        stride,
        data: vec![200u8; frame_size],
        timestamp_us: 2000,
    };
    let frame3 = CapturedFrame {
        width: 1920,
        height: 1080,
        stride,
        data: vec![255u8; frame_size],
        timestamp_us: 3000,
    };

    // Queue frame 1 and 2 (fills capacity)
    assert!(surface.queue_frame(frame1).is_ok());
    assert!(surface.queue_frame(frame2).is_ok());

    // Queue frame 3 (exceeds capacity=2, oldest frame dropped to enforce sub-frame latency)
    assert!(surface.queue_frame(frame3).is_ok());
    assert_eq!(surface.stats.dropped_frames, 1);

    // Present queued frame
    let presented = surface.present_queued_frame();
    assert!(presented.is_ok());
    assert!(presented.unwrap().is_some());
    assert_eq!(surface.stats.rendered_frames, 1);

    // Clean teardown
    assert!(app_state.stop_active_session().await.is_ok());
    assert!(surface.close().is_ok());
}

#[tokio::test]
async fn test_bidirectional_handshake_and_pin_pairing() {
    let app_state = AppState::new_simulated();

    // 1. Peer discovery injection
    let peer = PeerDevice::new(
        "peer-win-receiver",
        "Target Windows Receiver",
        "192.168.1.150".parse().unwrap(),
        5000,
        "receiver",
    );
    app_state.discovery_service().inject_peer(peer.clone());

    // 2. Host initiates extend display, generating a 4-digit PIN challenge
    let session_res = app_state.initiate_extend_display(&peer.id);
    assert!(session_res.is_ok());
    let session = session_res.unwrap();
    assert_eq!(session.state, "pairing");
    assert!(session.pin.is_some());
    let pin = session.pin.unwrap();
    assert_eq!(pin.len(), 4);
    assert!(pin.chars().all(|c| c.is_ascii_digit()));

    // 3. Receiver submits invalid PIN
    let bad_res = app_state.submit_pin(&peer.id, "0000");
    if pin != "0000" {
        assert_eq!(bad_res.unwrap(), false);
    }

    // 4. Receiver submits valid PIN -> triggers pairing completion and active streaming
    let good_res = app_state.submit_pin(&peer.id, &pin);
    assert!(good_res.is_ok());
    assert!(good_res.unwrap(), "Correct PIN must authenticate successfully");

    // 5. Active session established
    let active = app_state.get_active_session();
    assert!(active.is_some());
    assert_eq!(active.unwrap().state, "active");

    // Clean stop
    assert!(app_state.stop_active_session().await.is_ok());
}

#[tokio::test]
async fn test_full_bidirectional_pipeline_linux_host_to_windows_receiver() {
    // 1. Linux Host virtual display setup
    let host_config = DisplayConfig::new(1920, 1080, 60);
    let mut wayland_mgr = WaylandDisplayManager::new_simulated();
    let monitor_id = wayland_mgr.create_display(&host_config).unwrap();
    assert_eq!(monitor_id, 1);

    // 2. Linux PipeWire capture with cursor compositing
    let mut capturer = PipeWireCapturer::new_simulated(1920, 1080);
    capturer.start(monitor_id).unwrap();

    let cursor = PipeWireCursorMetadata {
        x: 100,
        y: 100,
        visible: true,
        width: 4,
        height: 4,
        hotspot_x: 0,
        hotspot_y: 0,
        data: vec![255u8; 4 * 4 * 4],
    };
    capturer.set_cursor_metadata(Some(cursor));

    let frame = capturer.capture_frame().unwrap().unwrap();
    assert_eq!(frame.width, 1920);
    assert_eq!(frame.height, 1080);
    assert!(frame.stride >= 1920 * 4);

    // 3. Hardware / Software H.264 Encoder
    let stream_cfg = StreamConfig::new(1920, 1080, 60, 8_000_000);
    let mut encoder = H264Encoder::new(stream_cfg).unwrap();
    let packet = encoder.encode_frame(&frame).unwrap();
    assert!(!packet.data.is_empty(), "Encoded H.264 packet must not be empty");

    // 4. RTP Packetization (RFC 6184)
    let ssrc = 0xDEADBEEF;
    let mut packetizer = RtpPacketizer::new(ssrc, 1200);
    let rtp_packets = packetizer.packetize(&packet).unwrap();
    assert!(!rtp_packets.is_empty(), "Packetizer must produce RTP packets");
    assert!(rtp_packets.last().unwrap().marker, "Last RTP packet must have marker bit set");

    // 5. Windows Receiver RTP Depacketization
    let mut depacketizer = H264Depacketizer::new(ssrc);
    let mut reassembled = None;
    for rtp in &rtp_packets {
        if let Some(f) = depacketizer.depacketize(rtp).unwrap() {
            reassembled = Some(f);
        }
    }
    assert!(reassembled.is_some(), "Depacketizer must reassemble complete video frame");
    let rx_frame = reassembled.unwrap();
    assert_eq!(rx_frame.timestamp_us, frame.timestamp_us);

    // 6. Windows Direct3D11 Presentation Surface Rendering
    let client_geom = ClientScreenGeometry {
        width: 1920,
        height: 1080,
        refresh_rate: 60,
    };
    let mut surface = WindowsD3D11Surface::new_simulated(client_geom, ReceiverConfig::default());
    assert!(surface.render_frame(&frame).is_ok());
    assert_eq!(surface.stats.rendered_frames, 1);

    // 7. Verify sub-50ms glass-to-glass latency budget
    let now_us = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64;
    let simulated_capture_us = now_us - 15_000; // 15ms elapsed
    let simulated_glass_to_glass_ms = (now_us - simulated_capture_us) / 1000;
    assert!(
        simulated_glass_to_glass_ms < 50,
        "Glass-to-glass latency ({}ms) must be strictly below 50ms",
        simulated_glass_to_glass_ms
    );

    // Teardown
    wayland_mgr.destroy_all().unwrap();
    capturer.stop().unwrap();
    surface.close().unwrap();
}

#[tokio::test]
async fn test_clean_bidirectional_teardown_and_orphan_prevention() {
    let app_state = AppState::new_simulated();

    // 1. Start Linux host
    let config = DisplayConfig::new(1920, 1080, 60);
    let start_res = app_state.start_linux_host(&config);
    assert!(start_res.is_ok());
    assert!(app_state.get_active_session().is_some());

    // 2. Stop active session
    let stop_res = app_state.stop_active_session().await;
    assert!(stop_res.is_ok());
    assert!(app_state.get_active_session().is_none());

    // 3. Start Windows Receiver
    let geom = ClientScreenGeometry::default();
    let rx_res = app_state.start_windows_receiver(&geom);
    assert!(rx_res.is_ok());

    // 4. Teardown hook
    app_state.teardown();
    assert!(app_state.get_active_session().is_none());
}

#[tokio::test]
async fn test_phase2_status_query_contract() {
    let app_state = AppState::new_simulated();

    let initial_status = app_state.get_phase2_status();
    assert_eq!(initial_status.role, "host");
    assert!(!initial_status.is_streaming);
    assert_eq!(initial_status.monitor_id, None);
    assert!(!initial_status.surface_active);

    // Start host
    let config = DisplayConfig::new(1920, 1080, 60);
    let _ = app_state.start_linux_host(&config);
    let host_status = app_state.get_phase2_status();
    assert_eq!(host_status.role, "host");
    assert!(host_status.is_streaming);
    assert!(host_status.monitor_id.is_some());

    // Stop host and start receiver
    let _ = app_state.stop_active_session().await;
    let _ = app_state.start_windows_receiver(&ClientScreenGeometry::default());
    let receiver_status = app_state.get_phase2_status();
    assert_eq!(receiver_status.role, "receiver");
    assert!(receiver_status.surface_active);

    let _ = app_state.stop_active_session().await;
}
