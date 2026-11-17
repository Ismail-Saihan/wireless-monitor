//! Unit tests for Windows Receiver Direct3D11 Presentation Surface
//!
//! Tests WindowsD3D11Surface initialization, D3D11 dynamic texture allocation,
//! sub-frame latency presentation queue (<1 frame stale frame drop logic),
//! dynamic resolution resizing, borderless fullscreen toggling, and frame rendering.

use wireless_monitor_lib::capture::CapturedFrame;
use wireless_monitor_lib::receiver::{
    ClientScreenGeometry, ReceiverConfig, SurfaceStats, WindowsD3D11Surface,
};

#[test]
fn test_windows_d3d11_surface_initialization_simulated() {
    let geometry = ClientScreenGeometry::new(1920, 1080, 60).expect("Valid geometry");
    let config = ReceiverConfig {
        fullscreen: true,
        target_display_id: None,
    };

    let surface = WindowsD3D11Surface::new_simulated(geometry, config);

    assert!(surface.is_simulated());
    assert!(surface.is_active());
    assert!(surface.is_fullscreen());
    assert_eq!(surface.geometry().width, 1920);
    assert_eq!(surface.geometry().height, 1080);
    assert_eq!(surface.geometry().refresh_rate, 60);

    let stats = surface.stats();
    assert_eq!(stats.rendered_frames, 0);
    assert_eq!(stats.dropped_frames, 0);
    assert_eq!(stats.queued_frames, 0);
    assert!(stats.is_fullscreen);

    assert_eq!(surface.texture_width(), 1920);
    assert_eq!(surface.texture_height(), 1080);
    assert_eq!(surface.texture_format(), "DXGI_FORMAT_B8G8R8A8_UNORM");
}

#[test]
fn test_windows_d3d11_surface_geometry_validation() {
    let config = ReceiverConfig::default();

    // Invalid zero width
    let invalid_zero_w = ClientScreenGeometry {
        width: 0,
        height: 1080,
        refresh_rate: 60,
    };
    assert!(WindowsD3D11Surface::new(invalid_zero_w, config.clone()).is_err());

    // Invalid zero height
    let invalid_zero_h = ClientScreenGeometry {
        width: 1920,
        height: 0,
        refresh_rate: 60,
    };
    assert!(WindowsD3D11Surface::new(invalid_zero_h, config.clone()).is_err());

    // Invalid zero refresh rate
    let invalid_zero_rr = ClientScreenGeometry {
        width: 1920,
        height: 1080,
        refresh_rate: 0,
    };
    assert!(WindowsD3D11Surface::new(invalid_zero_rr, config.clone()).is_err());

    // Invalid excessive refresh rate (>360)
    let invalid_excess_rr = ClientScreenGeometry {
        width: 1920,
        height: 1080,
        refresh_rate: 480,
    };
    assert!(WindowsD3D11Surface::new(invalid_excess_rr, config).is_err());
}

#[test]
fn test_windows_d3d11_surface_dynamic_texture_allocation() {
    let geometry = ClientScreenGeometry::new(2560, 1440, 120).expect("Valid 1440p geometry");
    let config = ReceiverConfig::default();

    let surface = WindowsD3D11Surface::new_simulated(geometry, config);

    assert_eq!(surface.texture_width(), 2560);
    assert_eq!(surface.texture_height(), 1440);
    assert_eq!(surface.texture_format(), "DXGI_FORMAT_B8G8R8A8_UNORM");

    // Texture buffer size should be width * height * 4
    let expected_bytes = 2560 * 1440 * 4;
    let texture_data = surface
        .uploaded_texture_data()
        .expect("Simulated texture buffer allocated");
    assert_eq!(texture_data.len(), expected_bytes as usize);
}

#[test]
fn test_windows_d3d11_surface_direct_frame_render() {
    let geometry = ClientScreenGeometry::new(1920, 1080, 60).expect("Valid geometry");
    let config = ReceiverConfig::default();
    let mut surface = WindowsD3D11Surface::new_simulated(geometry, config);

    let stride = 1920 * 4;
    let data_len = (stride * 1080) as usize;
    let mut frame_data = vec![0x80u8; data_len];
    // Write distinct header in first 4 bytes: BGRA (0x11, 0x22, 0x33, 0xFF)
    frame_data[0] = 0x11;
    frame_data[1] = 0x22;
    frame_data[2] = 0x33;
    frame_data[3] = 0xFF;

    let frame = CapturedFrame {
        width: 1920,
        height: 1080,
        stride,
        data: frame_data,
        timestamp_us: 100_000,
    };

    let render_res = surface.render_frame(&frame);
    assert!(render_res.is_ok());

    let stats = surface.stats();
    assert_eq!(stats.rendered_frames, 1);
    assert_eq!(stats.dropped_frames, 0);

    let last_frame = surface
        .last_rendered_frame()
        .expect("Last rendered frame available");
    assert_eq!(last_frame.timestamp_us, 100_000);
    assert_eq!(last_frame.width, 1920);

    // Verify texture upload buffer updated
    let tex = surface
        .uploaded_texture_data()
        .expect("Texture buffer available");
    assert_eq!(tex[0], 0x11);
    assert_eq!(tex[1], 0x22);
    assert_eq!(tex[2], 0x33);
    assert_eq!(tex[3], 0xFF);
}

#[test]
fn test_windows_d3d11_surface_sub_frame_latency_queue_and_dropping() {
    let geometry = ClientScreenGeometry::new(1920, 1080, 60).expect("Valid geometry");
    let config = ReceiverConfig::default();
    let mut surface = WindowsD3D11Surface::new_simulated(geometry, config);

    // Set queue capacity to 2 (guarantees <1 frame latency by dropping stale frames)
    surface.set_queue_capacity(2);
    assert_eq!(surface.queue_capacity(), 2);

    let stride = 1920 * 4;
    let data_len = (stride * 1080) as usize;

    let make_frame = |ts: u64, tag: u8| -> CapturedFrame {
        let mut d = vec![tag; data_len];
        d[0] = tag;
        CapturedFrame {
            width: 1920,
            height: 1080,
            stride,
            data: d,
            timestamp_us: ts,
        }
    };

    // Queue frame 1
    assert!(surface.queue_frame(make_frame(1, 0x01)).is_ok());
    assert_eq!(surface.stats().queued_frames, 1);
    assert_eq!(surface.stats().dropped_frames, 0);

    // Queue frame 2
    assert!(surface.queue_frame(make_frame(2, 0x02)).is_ok());
    assert_eq!(surface.stats().queued_frames, 2);
    assert_eq!(surface.stats().dropped_frames, 0);

    // Queue frame 3: Queue capacity 2 is exceeded!
    // Stale frame 1 must be dropped immediately to maintain sub-frame latency.
    assert!(surface.queue_frame(make_frame(3, 0x03)).is_ok());
    assert_eq!(surface.stats().queued_frames, 2);
    assert_eq!(surface.stats().dropped_frames, 1);

    // Queue frame 4: Stale frame 2 dropped.
    assert!(surface.queue_frame(make_frame(4, 0x04)).is_ok());
    assert_eq!(surface.stats().queued_frames, 2);
    assert_eq!(surface.stats().dropped_frames, 2);

    // Present queued frame: Should present frame 3
    let presented1 = surface
        .present_queued_frame()
        .expect("Presentation succeeds")
        .expect("Frame 3 present");
    assert_eq!(presented1.timestamp_us, 3);
    assert_eq!(surface.stats().rendered_frames, 1);
    assert_eq!(surface.stats().queued_frames, 1);

    // Present queued frame: Should present frame 4
    let presented2 = surface
        .present_queued_frame()
        .expect("Presentation succeeds")
        .expect("Frame 4 present");
    assert_eq!(presented2.timestamp_us, 4);
    assert_eq!(surface.stats().rendered_frames, 2);
    assert_eq!(surface.stats().queued_frames, 0);

    // Present when queue is empty
    let empty_res = surface
        .present_queued_frame()
        .expect("Presentation on empty succeeds");
    assert!(empty_res.is_none());
    assert_eq!(surface.stats().rendered_frames, 2);
}

#[test]
fn test_windows_d3d11_surface_resolution_resize() {
    let geometry = ClientScreenGeometry::new(1920, 1080, 60).expect("Valid geometry");
    let config = ReceiverConfig::default();
    let mut surface = WindowsD3D11Surface::new_simulated(geometry, config);

    assert_eq!(surface.geometry().width, 1920);
    assert_eq!(surface.geometry().height, 1080);
    assert_eq!(surface.texture_width(), 1920);

    // Resize to 4K resolution (3840x2160)
    assert!(surface.resize(3840, 2160).is_ok());
    assert_eq!(surface.geometry().width, 3840);
    assert_eq!(surface.geometry().height, 2160);
    assert_eq!(surface.texture_width(), 3840);
    assert_eq!(surface.texture_height(), 2160);

    let tex = surface
        .uploaded_texture_data()
        .expect("Texture buffer available");
    assert_eq!(tex.len(), (3840 * 2160 * 4) as usize);

    // Render a 4K frame to verify new dimensions accept 4K frames
    let frame_4k = CapturedFrame {
        width: 3840,
        height: 2160,
        stride: 3840 * 4,
        data: vec![0xAA; (3840 * 2160 * 4) as usize],
        timestamp_us: 200_000,
    };
    assert!(surface.render_frame(&frame_4k).is_ok());
    assert_eq!(surface.stats().rendered_frames, 1);

    // Invalid zero resize dimensions
    assert!(surface.resize(0, 2160).is_err());
    assert!(surface.resize(3840, 0).is_err());
    // Excessive dimensions (>8K)
    assert!(surface.resize(8000, 5000).is_err());
}

#[test]
fn test_windows_d3d11_surface_borderless_fullscreen_toggling() {
    let geometry = ClientScreenGeometry::new(1920, 1080, 60).expect("Valid geometry");
    let config = ReceiverConfig {
        fullscreen: true,
        target_display_id: None,
    };
    let mut surface = WindowsD3D11Surface::new_simulated(geometry, config);

    assert!(surface.is_fullscreen());
    assert!(surface.stats().is_fullscreen);

    // Toggle to windowed mode
    assert!(surface.set_fullscreen(false).is_ok());
    assert!(!surface.is_fullscreen());
    assert!(!surface.stats().is_fullscreen);

    // Toggle back to borderless fullscreen
    assert!(surface.set_fullscreen(true).is_ok());
    assert!(surface.is_fullscreen());
    assert!(surface.stats().is_fullscreen);
}

#[test]
fn test_windows_d3d11_surface_invalid_frame_rejection() {
    let geometry = ClientScreenGeometry::new(1920, 1080, 60).expect("Valid geometry");
    let config = ReceiverConfig::default();
    let mut surface = WindowsD3D11Surface::new_simulated(geometry, config);

    // Invalid frame: truncated data
    let invalid_frame1 = CapturedFrame {
        width: 1920,
        height: 1080,
        stride: 1920 * 4,
        data: vec![0u8; 100], // Far too short
        timestamp_us: 1,
    };
    assert!(surface.render_frame(&invalid_frame1).is_err());
    assert!(surface.queue_frame(invalid_frame1).is_err());

    // Invalid frame: stride < width * 4
    let invalid_frame2 = CapturedFrame {
        width: 1920,
        height: 1080,
        stride: 1920, // Less than width * 4
        data: vec![0u8; (1920 * 4 * 1080) as usize],
        timestamp_us: 2,
    };
    assert!(surface.render_frame(&invalid_frame2).is_err());

    // Invalid frame: dimension mismatch with surface geometry
    let invalid_frame3 = CapturedFrame {
        width: 1280,
        height: 720,
        stride: 1280 * 4,
        data: vec![0u8; (1280 * 4 * 720) as usize],
        timestamp_us: 3,
    };
    assert!(surface.render_frame(&invalid_frame3).is_err());
}

#[test]
fn test_windows_d3d11_surface_close_lifecycle() {
    let geometry = ClientScreenGeometry::new(1920, 1080, 60).expect("Valid geometry");
    let config = ReceiverConfig::default();
    let mut surface = WindowsD3D11Surface::new_simulated(geometry, config);

    assert!(surface.is_active());

    // Close surface
    assert!(surface.close().is_ok());
    assert!(!surface.is_active());

    // Attempting operations on closed surface must fail cleanly
    let frame = CapturedFrame {
        width: 1920,
        height: 1080,
        stride: 1920 * 4,
        data: vec![0u8; (1920 * 4 * 1080) as usize],
        timestamp_us: 1,
    };

    assert!(surface.render_frame(&frame).is_err());
    assert!(surface.queue_frame(frame).is_err());
    assert!(surface.resize(2560, 1440).is_err());
    assert!(surface.set_fullscreen(true).is_err());
}

#[test]
fn test_windows_d3d11_surface_thread_safety() {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    assert_send::<WindowsD3D11Surface>();
    assert_sync::<WindowsD3D11Surface>();
}
