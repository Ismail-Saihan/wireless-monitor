//! Unit tests for Linux PipeWire Screen Capture Engine

use wireless_monitor_lib::capture::pipewire::{
    calculate_aligned_stride, composite_pipewire_cursor, PipeWireCapturer, PipeWireCursorMetadata,
    PipeWireCursorMode, PipeWirePixelFormat,
};
use wireless_monitor_lib::capture::{CapturedFrame, ScreenCapturer};

#[test]
fn test_pipewire_capturer_initialization() {
    let capturer = PipeWireCapturer::new_simulated(1920, 1080);
    assert!(capturer.is_simulated());
    assert!(!capturer.is_running());
    assert_eq!(capturer.width(), 1920);
    assert_eq!(capturer.height(), 1080);
    assert_eq!(capturer.node_id(), None);
    assert_eq!(capturer.last_cursor_pos(), None);
    assert_eq!(capturer.cursor_mode(), PipeWireCursorMode::Metadata);

    // Verify direct field access
    assert_eq!(capturer.width, 1920);
    assert_eq!(capturer.height, 1080);
    assert_eq!(capturer.is_running, false);
    assert_eq!(capturer.is_simulated, true);
    assert_eq!(capturer.node_id, None);
    assert_eq!(capturer.last_cursor_pos, None);
}

#[test]
fn test_pipewire_capturer_from_node() {
    let capturer = PipeWireCapturer::from_node(42, 1920, 1080);
    assert_eq!(capturer.node_id(), Some(42));
    assert_eq!(capturer.node_id, Some(42));
    assert_eq!(capturer.width(), 1920);
    assert_eq!(capturer.height(), 1080);
    assert!(!capturer.is_running());
    assert!(!capturer.is_simulated());
}

#[test]
fn test_pipewire_capturer_lifecycle_not_started() {
    let mut capturer = PipeWireCapturer::new_simulated(1920, 1080);
    assert!(!capturer.is_running());

    // Capturing before starting must return Err
    let result = capturer.capture_frame(100);
    assert!(result.is_err(), "Capturing before start() must return Err");

    // Stopping before starting is a safe no-op
    assert!(capturer.stop().is_ok());
}

#[test]
fn test_pipewire_capturer_simulated_lifecycle() {
    let mut capturer = PipeWireCapturer::new_simulated(1920, 1080);
    assert!(capturer.start(10).is_ok());
    assert!(capturer.is_running());
    assert_eq!(capturer.node_id(), Some(10));

    let frame1 = capturer
        .capture_frame(100)
        .expect("Capture frame 1 should succeed")
        .expect("Frame should not be None");

    assert!(frame1.is_valid());
    assert_eq!(frame1.width, 1920);
    assert_eq!(frame1.height, 1080);
    assert!(frame1.stride >= 1920 * 4);
    assert_eq!(frame1.data.len(), frame1.buffer_size());

    let frame2 = capturer
        .capture_frame(100)
        .expect("Capture frame 2 should succeed")
        .expect("Frame 2 should not be None");

    assert!(frame2.timestamp_us > frame1.timestamp_us);

    assert!(capturer.stop().is_ok());
    assert!(!capturer.is_running());
}

#[test]
fn test_pipewire_stride_alignment() {
    assert_eq!(calculate_aligned_stride(1920, 4, 16), 7680);
    assert_eq!(calculate_aligned_stride(1921, 4, 16), 7696);
    assert_eq!(calculate_aligned_stride(1366, 4, 64), 5504);
    assert_eq!(calculate_aligned_stride(100, 3, 4), 300);
    assert_eq!(calculate_aligned_stride(101, 3, 4), 304);
}

#[test]
fn test_pipewire_buffer_conversion_to_rgba() {
    // 2x2 test image in BGRA format
    // Pixel (0,0): B=10, G=20, R=30, A=200
    // Pixel (1,0): B=40, G=50, R=60, A=255
    // Pixel (0,1): B=70, G=80, R=90, A=255
    // Pixel (1,1): B=100, G=110, R=120, A=255
    let bgra_data = vec![
        10, 20, 30, 200, 40, 50, 60, 255, 70, 80, 90, 255, 100, 110, 120, 255,
    ];

    let frame = PipeWireCapturer::ingest_raw_buffer(
        &bgra_data,
        2,
        2,
        8,
        PipeWirePixelFormat::BGRA,
    )
    .expect("Ingest BGRA buffer");

    assert!(frame.is_valid());
    assert_eq!(frame.get_pixel(0, 0), Some([30, 20, 10, 200]));
    assert_eq!(frame.get_pixel(1, 0), Some([60, 50, 40, 255]));

    // RGBx test: Alpha channel should be set to 255
    let rgbx_data = vec![15, 25, 35, 0, 45, 55, 65, 0, 75, 85, 95, 0, 105, 115, 125, 0];

    let frame_rgbx = PipeWireCapturer::ingest_raw_buffer(
        &rgbx_data,
        2,
        2,
        8,
        PipeWirePixelFormat::RGBx,
    )
    .expect("Ingest RGBx buffer");

    assert_eq!(frame_rgbx.get_pixel(0, 0), Some([15, 25, 35, 255]));

    // RGB 24-bit test: 3 bytes per pixel expanded to RGBA with alpha 255
    let rgb24_data = vec![11, 22, 33, 44, 55, 66, 77, 88, 99, 111, 122, 133];

    let frame_rgb24 = PipeWireCapturer::ingest_raw_buffer(
        &rgb24_data,
        2,
        2,
        6,
        PipeWirePixelFormat::RGB,
    )
    .expect("Ingest RGB 24-bit buffer");

    assert_eq!(frame_rgb24.get_pixel(0, 0), Some([11, 22, 33, 255]));
}

#[test]
fn test_pipewire_cursor_metadata_extraction_and_alpha_compositing() {
    let mut frame_data = vec![0u8; 10 * 10 * 4];
    // Fill with solid blue: R=0, G=0, B=255, A=255
    for i in 0..100 {
        frame_data[i * 4 + 2] = 255;
        frame_data[i * 4 + 3] = 255;
    }

    // 2x2 cursor metadata at (2, 2)
    // Pixel (0,0): semi-transparent red [255, 0, 0, 128]
    // Pixel (1,0): fully opaque green [0, 255, 0, 255]
    // Pixel (0,1): transparent [0, 0, 0, 0]
    // Pixel (1,1): white [255, 255, 255, 255]
    let cursor = PipeWireCursorMetadata {
        x: 2,
        y: 2,
        visible: true,
        width: 2,
        height: 2,
        hotspot_x: 0,
        hotspot_y: 0,
        data: vec![
            255, 0, 0, 128, 0, 255, 0, 255, 0, 0, 0, 0, 255, 255, 255, 255,
        ],
    };

    composite_pipewire_cursor(&mut frame_data, 10, 10, 40, &cursor)
        .expect("Cursor compositing should succeed");

    // Pixel (2,2) - semi-transparent red blended over blue
    let idx_2_2 = (2 * 40) + (2 * 4);
    assert_eq!(frame_data[idx_2_2], 128); // (255 * 128 + 0 * 127) / 255 = 128
    assert_eq!(frame_data[idx_2_2 + 1], 0);
    assert_eq!(frame_data[idx_2_2 + 2], 127); // (0 * 128 + 255 * 127) / 255 = 127
    assert_eq!(frame_data[idx_2_2 + 3], 255);

    // Pixel (3,2) - opaque green
    let idx_3_2 = (2 * 40) + (3 * 4);
    assert_eq!(frame_data[idx_3_2], 0);
    assert_eq!(frame_data[idx_3_2 + 1], 255);
    assert_eq!(frame_data[idx_3_2 + 2], 0);
    assert_eq!(frame_data[idx_3_2 + 3], 255);

    // Pixel (2,3) - transparent cursor, remains solid blue
    let idx_2_3 = (3 * 40) + (2 * 4);
    assert_eq!(frame_data[idx_2_3], 0);
    assert_eq!(frame_data[idx_2_3 + 1], 0);
    assert_eq!(frame_data[idx_2_3 + 2], 255);
    assert_eq!(frame_data[idx_2_3 + 3], 255);

    // Untouched pixel (0,0) - solid blue
    assert_eq!(frame_data[0], 0);
    assert_eq!(frame_data[1], 0);
    assert_eq!(frame_data[2], 255);
    assert_eq!(frame_data[3], 255);
}

#[test]
fn test_pipewire_cursor_boundary_clipping() {
    let mut frame_data = vec![0u8; 10 * 10 * 4];

    // Cursor 4x4 placed partially off top-left at (-2, -2)
    let cursor_top_left = PipeWireCursorMetadata {
        x: -2,
        y: -2,
        visible: true,
        width: 4,
        height: 4,
        hotspot_x: 0,
        hotspot_y: 0,
        data: vec![255; 4 * 4 * 4],
    };

    assert!(composite_pipewire_cursor(&mut frame_data, 10, 10, 40, &cursor_top_left).is_ok());

    // Cursor placed completely outside screen bounds
    let cursor_outside = PipeWireCursorMetadata {
        x: 100,
        y: 100,
        visible: true,
        width: 4,
        height: 4,
        hotspot_x: 0,
        hotspot_y: 0,
        data: vec![255; 4 * 4 * 4],
    };

    assert!(composite_pipewire_cursor(&mut frame_data, 10, 10, 40, &cursor_outside).is_ok());
}

#[test]
fn test_pipewire_cursor_stationary_persistence() {
    let mut capturer = PipeWireCapturer::new_simulated(100, 100);
    capturer.start(1).unwrap();

    let cursor = PipeWireCursorMetadata {
        x: 50,
        y: 60,
        visible: true,
        width: 2,
        height: 2,
        hotspot_x: 0,
        hotspot_y: 0,
        data: vec![255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255],
    };

    capturer.set_cursor_metadata(Some(cursor));
    capturer.set_cursor_position(50, 60);

    let frame1 = capturer.capture_frame(50).unwrap().unwrap();
    assert_eq!(capturer.last_cursor_pos(), Some((50, 60)));
    assert_eq!(frame1.get_pixel(50, 60), Some([255, 0, 0, 255]));

    // Second capture without updated cursor metadata: stationary cursor persists!
    capturer.set_cursor_metadata(None);
    let frame2 = capturer.capture_frame(50).unwrap().unwrap();
    assert_eq!(capturer.last_cursor_pos(), Some((50, 60)));
    assert_eq!(frame2.get_pixel(50, 60), Some([255, 0, 0, 255]));
}

#[test]
fn test_pipewire_cursor_hidden_and_embedded_modes() {
    let mut capturer = PipeWireCapturer::new_simulated(50, 50);
    capturer.start(1).unwrap();

    let cursor = PipeWireCursorMetadata {
        x: 10,
        y: 10,
        visible: true,
        width: 2,
        height: 2,
        hotspot_x: 0,
        hotspot_y: 0,
        data: [255, 0, 0, 255].repeat(4),
    };
    capturer.set_cursor_metadata(Some(cursor));

    // Hidden mode: cursor should not be drawn
    capturer.set_cursor_mode(PipeWireCursorMode::Hidden);
    let frame_hidden = capturer.capture_frame(50).unwrap().unwrap();
    assert_ne!(frame_hidden.get_pixel(10, 10), Some([255, 0, 0, 255]));

    // Embedded mode: capturer assumes cursor is already embedded in compositor stream
    capturer.set_cursor_mode(PipeWireCursorMode::Embedded);
    let frame_embedded = capturer.capture_frame(50).unwrap().unwrap();
    assert_ne!(frame_embedded.get_pixel(10, 10), Some([255, 0, 0, 255]));

    // Metadata mode: software cursor composited
    capturer.set_cursor_mode(PipeWireCursorMode::Metadata);
    let frame_meta = capturer.capture_frame(50).unwrap().unwrap();
    assert_eq!(frame_meta.get_pixel(10, 10), Some([255, 0, 0, 255]));
}

#[test]
fn test_pipewire_capturer_timeout_handling() {
    let mut capturer = PipeWireCapturer::new_simulated(100, 100);
    capturer.start(1).unwrap();

    capturer.set_simulated_timeout(true);
    let timeout_res = capturer.capture_frame(50).unwrap();
    assert_eq!(timeout_res, None);

    capturer.set_simulated_timeout(false);
    let ok_res = capturer.capture_frame(50).unwrap();
    assert!(ok_res.is_some());
}

#[test]
fn test_pipewire_capturer_stream_termination_and_drop() {
    let mut capturer = PipeWireCapturer::new_simulated(100, 100);
    assert!(capturer.start(1).is_ok());
    assert!(capturer.is_running());
    assert_eq!(capturer.node_id(), Some(1));

    assert!(capturer.stop().is_ok());
    assert!(!capturer.is_running());
    assert_eq!(capturer.node_id(), None);

    // Restarting after stop succeeds
    assert!(capturer.start(2).is_ok());
    assert!(capturer.is_running());
    assert_eq!(capturer.node_id(), Some(2));
}

#[test]
fn test_pipewire_capturer_resolution_validation() {
    let mut capturer = PipeWireCapturer::new_simulated(1920, 1080);
    assert!(capturer.set_simulated_resolution(0, 1080).is_err());
    assert!(capturer.set_simulated_resolution(1920, 0).is_err());
    assert!(capturer.set_simulated_resolution(10000, 5000).is_err());

    assert!(capturer.set_simulated_resolution(2560, 1440).is_ok());
    assert_eq!(capturer.width(), 2560);
    assert_eq!(capturer.height(), 1440);
}

#[test]
fn test_real_pipewire_on_non_linux() {
    #[cfg(not(target_os = "linux"))]
    {
        let mut capturer = PipeWireCapturer::from_node(42, 1920, 1080);
        assert!(!capturer.is_simulated());
        let res = capturer.start(42);
        assert!(res.is_err());
        let err_msg = res.unwrap_err().to_string();
        assert!(err_msg.contains("Linux"));
    }
}

#[test]
fn test_ingest_raw_buffer_invalid_stride_rejected() {
    // 10x10 RGBA image requires src_stride >= 10 * 4 = 40
    let truncated_stride_data = vec![0u8; 30 * 10];
    let res = PipeWireCapturer::ingest_raw_buffer(
        &truncated_stride_data,
        10,
        10,
        30, // Invalid: less than width * 4
        PipeWirePixelFormat::RGBA,
    );
    assert!(res.is_err());
    let err_msg = res.unwrap_err().to_string();
    assert!(err_msg.contains("smaller than minimum required stride"));

    // 10x10 RGB 24-bit requires src_stride >= 10 * 3 = 30
    let res_rgb = PipeWireCapturer::ingest_raw_buffer(
        &truncated_stride_data,
        10,
        10,
        25, // Invalid: less than width * 3
        PipeWirePixelFormat::RGB,
    );
    assert!(res_rgb.is_err());
    assert!(res_rgb.unwrap_err().to_string().contains("smaller than minimum required stride"));
}

#[test]
fn test_composite_cursor_if_needed() {
    let mut capturer = PipeWireCapturer::new_simulated(50, 50);
    capturer.start(1).unwrap();

    let mut frame_buf = vec![0u8; 50 * 50 * 4];

    // Hidden mode: does not composite
    capturer.set_cursor_mode(PipeWireCursorMode::Hidden);
    let cursor = PipeWireCursorMetadata {
        x: 5,
        y: 5,
        visible: true,
        width: 2,
        height: 2,
        hotspot_x: 0,
        hotspot_y: 0,
        data: [255, 0, 0, 255].repeat(4),
    };
    capturer.set_cursor_metadata(Some(cursor.clone()));
    assert!(capturer.composite_cursor_if_needed(&mut frame_buf, 50 * 4).is_ok());
    assert_ne!(frame_buf[(5 * 50 * 4) + (5 * 4)], 255);

    // Metadata mode: composites active cursor
    capturer.set_cursor_mode(PipeWireCursorMode::Metadata);
    assert!(capturer.composite_cursor_if_needed(&mut frame_buf, 50 * 4).is_ok());
    assert_eq!(frame_buf[(5 * 50 * 4) + (5 * 4)], 255);

    // Stationary cursor: when metadata is None, cached cursor persists
    capturer.set_cursor_metadata(None);
    let mut frame_buf2 = vec![0u8; 50 * 50 * 4];
    assert!(capturer.composite_cursor_if_needed(&mut frame_buf2, 50 * 4).is_ok());
    assert_eq!(frame_buf2[(5 * 50 * 4) + (5 * 4)], 255);
}

#[test]
fn test_pipewire_capturer_new() {
    let mut capturer = PipeWireCapturer::new(1280, 720);
    assert_eq!(capturer.width(), 1280);
    assert_eq!(capturer.height(), 720);
    assert_eq!(capturer.node_id(), None);

    #[cfg(not(target_os = "linux"))]
    {
        // On non-Linux, new() defaults to simulated mode
        assert!(capturer.is_simulated());
        assert!(capturer.start(3).is_ok());
        assert_eq!(capturer.node_id(), Some(3));
    }
}

#[test]
fn test_pipewire_capturer_thread_safety() {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}
    assert_send::<PipeWireCapturer>();
    assert_sync::<PipeWireCapturer>();
}
