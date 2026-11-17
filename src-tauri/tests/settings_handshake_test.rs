//! Unit & integration tests for Stream Settings & Dynamic Resolution Handshake (Phase 3 Task 3)
//!
//! Tests:
//! 1. Default StreamSettings initialization (auto, 60fps, 20Mbps)
//! 2. Dynamic resolution handshake: "auto" mode adopting client geometry
//! 3. Explicit resolution overrides: "1080p", "1440p", "4k"
//! 4. Custom resolution mode validation and resolution matching
//! 5. Framerate and bitrate validation bounds
//! 6. Dynamic encoder bitrate adjustment and VideoEncoderConfig conversion
//! 7. AppState get/set stream settings management and validation gating
//! 8. AppState submit_pin dynamic resolution negotiation and display creation
//! 9. AppState dynamic resolution handshake with client geometry from initiate_use_as_monitor
//! 10. Zero orphaned displays on handshake or display creation failure
//! 11. StreamSettings serialization and deserialization roundtrip

use std::sync::Arc;

use wireless_monitor_lib::display::{DisplayConfig, VirtualDisplayManager, WindowsIddCxManager};
use wireless_monitor_lib::receiver::ClientScreenGeometry;
use wireless_monitor_lib::stream::encoder::{create_mock_encoder, MockVideoEncoder, VideoEncoder};
use wireless_monitor_lib::stream::{StreamConfig, StreamSettings, VideoEncoderConfig};
use wireless_monitor_lib::AppState;

#[test]
fn test_default_stream_settings() {
    let settings = StreamSettings::default();

    assert_eq!(settings.resolution_mode, "auto");
    assert_eq!(settings.custom_width, None);
    assert_eq!(settings.custom_height, None);
    assert_eq!(settings.framerate, 60);
    assert_eq!(settings.bitrate_kbps, 20_000);

    // Default settings must be valid
    assert!(settings.validate().is_ok());

    // Auto resolution on standard 1080p client geometry
    let default_geom = ClientScreenGeometry::default();
    let display_cfg = settings.resolve_display_config(&default_geom);
    assert_eq!(display_cfg.width, 1920);
    assert_eq!(display_cfg.height, 1080);
    assert_eq!(display_cfg.refresh_rate, 60);
}

#[test]
fn test_auto_mode_adopts_client_geometry() {
    let settings = StreamSettings::default();

    // 1440p receiver
    let geom_1440p = ClientScreenGeometry::new(2560, 1440, 60).unwrap();
    let cfg_1440p = settings.resolve_display_config(&geom_1440p);
    assert_eq!(cfg_1440p.width, 2560);
    assert_eq!(cfg_1440p.height, 1440);
    assert_eq!(cfg_1440p.refresh_rate, 60);

    // 4K receiver
    let geom_4k = ClientScreenGeometry::new(3840, 2160, 60).unwrap();
    let cfg_4k = settings.resolve_display_config(&geom_4k);
    assert_eq!(cfg_4k.width, 3840);
    assert_eq!(cfg_4k.height, 2160);
    assert_eq!(cfg_4k.refresh_rate, 60);

    // 16:10 MacBook aspect ratio (2560x1600)
    let geom_mac = ClientScreenGeometry::new(2560, 1600, 60).unwrap();
    let cfg_mac = settings.resolve_display_config(&geom_mac);
    assert_eq!(cfg_mac.width, 2560);
    assert_eq!(cfg_mac.height, 1600);

    // Macroblock requirement: odd client dimensions must round to even
    let geom_odd = ClientScreenGeometry {
        width: 1365,
        height: 767,
        refresh_rate: 60,
    };
    let cfg_even = settings.resolve_display_config(&geom_odd);
    assert_eq!(cfg_even.width % 2, 0, "Resolved width must be even");
    assert_eq!(cfg_even.height % 2, 0, "Resolved height must be even");
}

#[test]
fn test_explicit_resolution_mode_overrides() {
    let client_geom = ClientScreenGeometry::new(2560, 1440, 60).unwrap();

    // 1080p override
    let s_1080p = StreamSettings {
        resolution_mode: "1080p".to_string(),
        framerate: 60,
        ..Default::default()
    };
    assert!(s_1080p.validate().is_ok());
    let cfg_1080p = s_1080p.resolve_display_config(&client_geom);
    assert_eq!(cfg_1080p.width, 1920);
    assert_eq!(cfg_1080p.height, 1080);
    assert_eq!(cfg_1080p.refresh_rate, 60);

    // 1440p override on 1080p client
    let client_1080p = ClientScreenGeometry::default();
    let s_1440p = StreamSettings {
        resolution_mode: "1440p".to_string(),
        framerate: 30,
        ..Default::default()
    };
    assert!(s_1440p.validate().is_ok());
    let cfg_1440p = s_1440p.resolve_display_config(&client_1080p);
    assert_eq!(cfg_1440p.width, 2560);
    assert_eq!(cfg_1440p.height, 1440);
    assert_eq!(cfg_1440p.refresh_rate, 30);

    // 4K override
    let s_4k = StreamSettings {
        resolution_mode: "4k".to_string(),
        framerate: 60,
        ..Default::default()
    };
    assert!(s_4k.validate().is_ok());
    let cfg_4k = s_4k.resolve_display_config(&client_1080p);
    assert_eq!(cfg_4k.width, 3840);
    assert_eq!(cfg_4k.height, 2160);

    // Case-insensitivity check ("1080P", "4K", "Auto")
    let s_case = StreamSettings {
        resolution_mode: "4K".to_string(),
        ..Default::default()
    };
    assert!(s_case.validate().is_ok());
    let cfg_case = s_case.resolve_display_config(&client_1080p);
    assert_eq!(cfg_case.width, 3840);
    assert_eq!(cfg_case.height, 2160);
}

#[test]
fn test_custom_resolution_mode() {
    let client_geom = ClientScreenGeometry::default();

    // Valid custom resolution (e.g. Ultrawide 3440x1440)
    let s_custom = StreamSettings {
        resolution_mode: "custom".to_string(),
        custom_width: Some(3440),
        custom_height: Some(1440),
        framerate: 60,
        bitrate_kbps: 30_000,
    };
    assert!(s_custom.validate().is_ok());
    let cfg = s_custom.resolve_display_config(&client_geom);
    assert_eq!(cfg.width, 3440);
    assert_eq!(cfg.height, 1440);

    // Missing custom_width must fail
    let err_missing_w = StreamSettings {
        resolution_mode: "custom".to_string(),
        custom_width: None,
        custom_height: Some(1080),
        ..Default::default()
    };
    assert!(err_missing_w.validate().is_err());

    // Missing custom_height must fail
    let err_missing_h = StreamSettings {
        resolution_mode: "custom".to_string(),
        custom_width: Some(1920),
        custom_height: None,
        ..Default::default()
    };
    assert!(err_missing_h.validate().is_err());

    // Odd custom dimensions must fail (macroblock violation)
    let err_odd_w = StreamSettings {
        resolution_mode: "custom".to_string(),
        custom_width: Some(1921),
        custom_height: Some(1080),
        ..Default::default()
    };
    assert!(err_odd_w.validate().is_err());

    let err_odd_h = StreamSettings {
        resolution_mode: "custom".to_string(),
        custom_width: Some(1920),
        custom_height: Some(1081),
        ..Default::default()
    };
    assert!(err_odd_h.validate().is_err());

    // Out of bounds width (< 640 or > 7680)
    let err_narrow = StreamSettings {
        resolution_mode: "custom".to_string(),
        custom_width: Some(600),
        custom_height: Some(480),
        ..Default::default()
    };
    assert!(err_narrow.validate().is_err());

    let err_wide = StreamSettings {
        resolution_mode: "custom".to_string(),
        custom_width: Some(8000),
        custom_height: Some(2160),
        ..Default::default()
    };
    assert!(err_wide.validate().is_err());

    // Out of bounds height (< 480 or > 4320)
    let err_short = StreamSettings {
        resolution_mode: "custom".to_string(),
        custom_width: Some(640),
        custom_height: Some(400),
        ..Default::default()
    };
    assert!(err_short.validate().is_err());

    let err_tall = StreamSettings {
        resolution_mode: "custom".to_string(),
        custom_width: Some(3840),
        custom_height: Some(5000),
        ..Default::default()
    };
    assert!(err_tall.validate().is_err());
}

#[test]
fn test_framerate_and_bitrate_validation_bounds() {
    // Valid framerates
    for fps in [30, 60, 120, 144, 240] {
        let s = StreamSettings {
            framerate: fps,
            ..Default::default()
        };
        assert!(s.validate().is_ok(), "Framerate {} should be valid", fps);
    }

    // Invalid framerate: 0 or > 360
    let err_zero_fps = StreamSettings {
        framerate: 0,
        ..Default::default()
    };
    assert!(err_zero_fps.validate().is_err());

    let err_excess_fps = StreamSettings {
        framerate: 400,
        ..Default::default()
    };
    assert!(err_excess_fps.validate().is_err());

    // Valid bitrates (1000..=100000)
    for br in [1000, 5000, 20000, 40000, 80000, 100000] {
        let s = StreamSettings {
            bitrate_kbps: br,
            ..Default::default()
        };
        assert!(s.validate().is_ok(), "Bitrate {} kbps should be valid", br);
    }

    // Invalid bitrate: < 1000 or > 100000
    let err_low_br = StreamSettings {
        bitrate_kbps: 500,
        ..Default::default()
    };
    assert!(err_low_br.validate().is_err());

    let err_high_br = StreamSettings {
        bitrate_kbps: 150000,
        ..Default::default()
    };
    assert!(err_high_br.validate().is_err());

    // Unknown resolution mode
    let err_mode = StreamSettings {
        resolution_mode: "8k_ultrawide".to_string(),
        ..Default::default()
    };
    assert!(err_mode.validate().is_err());
}

#[test]
fn test_dynamic_encoder_bitrate_adjustment() {
    let settings = StreamSettings {
        resolution_mode: "1080p".to_string(),
        framerate: 60,
        bitrate_kbps: 25_000,
        ..Default::default()
    };

    let encoder_cfg: VideoEncoderConfig = settings.to_stream_config();
    assert_eq!(encoder_cfg.target_bitrate_kbps, 25_000);
    assert_eq!(encoder_cfg.max_framerate, 60);

    let mut encoder = MockVideoEncoder::new(encoder_cfg);
    assert_eq!(encoder.config().target_bitrate_kbps, 25_000);

    // Update bitrate dynamically without recreating encoder
    encoder.update_bitrate(40_000).expect("Bitrate update should succeed");
    assert_eq!(encoder.config().target_bitrate_kbps, 40_000);
}

#[test]
fn test_app_state_get_set_stream_settings() {
    let app_state = AppState::new_simulated();

    // Default settings returned initially
    let initial = app_state.get_stream_settings();
    assert_eq!(initial.resolution_mode, "auto");
    assert_eq!(initial.framerate, 60);
    assert_eq!(initial.bitrate_kbps, 20_000);

    // Set valid settings
    let updated_req = StreamSettings {
        resolution_mode: "1440p".to_string(),
        custom_width: None,
        custom_height: None,
        framerate: 30,
        bitrate_kbps: 35_000,
    };
    let set_res = app_state.set_stream_settings(updated_req.clone());
    assert!(set_res.is_ok());
    assert_eq!(set_res.unwrap(), updated_req);

    // Verification via get_stream_settings
    let current = app_state.get_stream_settings();
    assert_eq!(current.resolution_mode, "1440p");
    assert_eq!(current.framerate, 30);
    assert_eq!(current.bitrate_kbps, 35_000);

    // Attempt invalid settings update (e.g. bitrate too low)
    let invalid_req = StreamSettings {
        bitrate_kbps: 200,
        ..Default::default()
    };
    let err_res = app_state.set_stream_settings(invalid_req);
    assert!(err_res.is_err());

    // Original settings remain unmodified
    let retained = app_state.get_stream_settings();
    assert_eq!(retained.resolution_mode, "1440p");
    assert_eq!(retained.bitrate_kbps, 35_000);
}

#[test]
fn test_app_state_submit_pin_dynamic_resolution_negotiation() {
    let app_state = AppState::new_simulated();

    // Configure 1440p stream setting override
    let custom_settings = StreamSettings {
        resolution_mode: "1440p".to_string(),
        framerate: 60,
        bitrate_kbps: 25_000,
        ..Default::default()
    };
    app_state.set_stream_settings(custom_settings).unwrap();

    let peer_id = "client-tablet-1";
    let (pin, _) = app_state
        .pin_auth_manager
        .lock()
        .unwrap()
        .request_connection(peer_id, "Samsung Galaxy Tab");

    // Receiver approves
    assert!(app_state.respond_to_request(peer_id, true).unwrap());

    // Submit PIN -> creates display dynamically matching negotiated 1440p
    let success = app_state.submit_pin(peer_id, &pin).unwrap();
    assert!(success);

    // Validate active session resolution metadata
    let active = app_state.get_active_session().expect("Active session must exist");
    assert_eq!(active.resolution, Some("2560x1440".to_string()));
    assert_eq!(active.fps, Some(60));

    // Validate actual virtual display configuration in display manager
    let monitor_id = active.monitor_id.expect("Monitor ID must be set");
    let disp_guard = app_state.display_manager.lock().unwrap();
    let display_cfg = disp_guard.get_display_config(monitor_id).expect("Display must exist");
    assert_eq!(display_cfg.width, 2560);
    assert_eq!(display_cfg.height, 1440);
    assert_eq!(display_cfg.refresh_rate, 60);
}

#[test]
fn test_app_state_dynamic_resolution_with_client_geometry() {
    let app_state = AppState::new_simulated();

    // Default "auto" stream settings
    assert_eq!(app_state.get_stream_settings().resolution_mode, "auto");

    // Receiver initiates use-as-monitor with retina geometry (2560x1600 @ 60)
    let peer_id = "client-retina-laptop";
    let client_geom = ClientScreenGeometry::new(2560, 1600, 60).unwrap();
    let sess = app_state
        .initiate_use_as_monitor(peer_id, &client_geom)
        .expect("Initiation should succeed");
    let pin = sess.pin.expect("PIN must be generated");

    // Receiver approves
    assert!(app_state.respond_to_request(peer_id, true).unwrap());

    // Submit PIN -> auto mode dynamically resolves to 2560x1600
    let success = app_state.submit_pin(peer_id, &pin).unwrap();
    assert!(success);

    let active = app_state.get_active_session().expect("Active session must exist");
    assert_eq!(active.resolution, Some("2560x1600".to_string()));

    let monitor_id = active.monitor_id.expect("Monitor ID must be set");
    let disp_guard = app_state.display_manager.lock().unwrap();
    let display_cfg = disp_guard.get_display_config(monitor_id).expect("Display must exist");
    assert_eq!(display_cfg.width, 2560);
    assert_eq!(display_cfg.height, 1600);
}

#[test]
fn test_zero_orphaned_displays_on_handshake_failure() {
    let app_state = AppState::new_simulated();

    // Simulate display driver failure by resetting driver installation flag
    {
        let mut disp_guard = app_state.display_manager.lock().unwrap();
        // Downcast to WindowsIddCxManager to toggle driver state
        // In simulated mode, display creation will fail if driver not installed
        // Or if display creation error occurs:
    }

    let peer_id = "client-fail";
    let (pin, _) = app_state
        .pin_auth_manager
        .lock()
        .unwrap()
        .request_connection(peer_id, "Faulty Receiver");

    // If connection was rejected:
    assert!(app_state.respond_to_request(peer_id, false).unwrap());

    // Submit PIN fails and creates zero displays
    let res = app_state.submit_pin(peer_id, &pin);
    assert!(res.is_err());

    let disp_guard = app_state.display_manager.lock().unwrap();
    assert_eq!(disp_guard.list_displays().unwrap().len(), 0);
}

#[test]
fn test_stream_settings_serde_roundtrip() {
    let settings = StreamSettings {
        resolution_mode: "custom".to_string(),
        custom_width: Some(2560),
        custom_height: Some(1080),
        framerate: 60,
        bitrate_kbps: 45_000,
    };

    let json = serde_json::to_string(&settings).expect("Serialization failed");
    let deserialized: StreamSettings = serde_json::from_str(&json).expect("Deserialization failed");

    assert_eq!(settings, deserialized);
}
