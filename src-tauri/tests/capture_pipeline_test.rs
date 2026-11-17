//! Unit and integration tests for DXGI Desktop Duplication Screen Capture Pipeline

use wireless_monitor_lib::capture::{
    composite_cursor, CapturedFrame, DxgiCapturer, PointerPosition, PointerShape,
    PointerShapeType, ScreenCapturer,
};

#[test]
fn test_captured_frame_validation_and_buffer_size() {
    let valid_frame = CapturedFrame {
        width: 1920,
        height: 1080,
        stride: 1920 * 4,
        data: vec![0u8; 1920 * 1080 * 4],
        timestamp_us: 1_000_000,
    };

    assert_eq!(valid_frame.buffer_size(), 1920 * 1080 * 4);
    assert!(valid_frame.is_valid(), "Valid frame should pass validation");

    let invalid_zero_width = CapturedFrame {
        width: 0,
        height: 1080,
        stride: 0,
        data: vec![],
        timestamp_us: 1_000_000,
    };
    assert!(!invalid_zero_width.is_valid(), "Zero width frame must be invalid");

    let invalid_zero_height = CapturedFrame {
        width: 1920,
        height: 0,
        stride: 1920 * 4,
        data: vec![],
        timestamp_us: 1_000_000,
    };
    assert!(!invalid_zero_height.is_valid(), "Zero height frame must be invalid");

    let invalid_small_stride = CapturedFrame {
        width: 1920,
        height: 1080,
        stride: 1920 * 2, // Less than width * 4 for 32-bit RGBA
        data: vec![0u8; 1920 * 1080 * 2],
        timestamp_us: 1_000_000,
    };
    assert!(!invalid_small_stride.is_valid(), "Under-sized stride must be invalid");

    let invalid_truncated_data = CapturedFrame {
        width: 1920,
        height: 1080,
        stride: 1920 * 4,
        data: vec![0u8; 100], // Truncated buffer
        timestamp_us: 1_000_000,
    };
    assert!(!invalid_truncated_data.is_valid(), "Truncated data buffer must be invalid");
}

#[test]
fn test_captured_frame_pixel_accessor() {
    let mut data = vec![0u8; 100 * 100 * 4];
    // Put a distinct pixel at (10, 20): R=255, G=128, B=64, A=255
    let offset = (20 * 100 * 4) + (10 * 4);
    data[offset] = 255;
    data[offset + 1] = 128;
    data[offset + 2] = 64;
    data[offset + 3] = 255;

    let frame = CapturedFrame {
        width: 100,
        height: 100,
        stride: 100 * 4,
        data,
        timestamp_us: 500,
    };

    assert_eq!(frame.get_pixel(10, 20), Some([255, 128, 64, 255]));
    assert_eq!(frame.get_pixel(0, 0), Some([0, 0, 0, 0]));
    // Out of bounds checks
    assert_eq!(frame.get_pixel(100, 20), None);
    assert_eq!(frame.get_pixel(10, 100), None);
    assert_eq!(frame.get_pixel(999, 999), None);
}

#[test]
fn test_screen_capturer_lifecycle_not_started() {
    let mut capturer = DxgiCapturer::new_simulated();
    assert!(!capturer.is_running());
    assert_eq!(capturer.output_index(), None);

    // Capturing before starting must return Err
    let result = capturer.capture_frame(100);
    assert!(result.is_err(), "Capturing before start() must return Err");

    // Stopping before starting is a safe no-op
    assert!(capturer.stop().is_ok());
}

#[test]
fn test_screen_capturer_simulated_lifecycle() {
    let mut capturer = DxgiCapturer::new_simulated();
    assert!(capturer.is_simulated());

    // Start capturer on output 0
    let start_res = capturer.start(0);
    assert!(start_res.is_ok(), "start(0) should succeed in simulated mode");
    assert!(capturer.is_running());
    assert_eq!(capturer.output_index(), Some(0));

    // Capture first frame
    let frame1_opt = capturer.capture_frame(50).expect("Frame capture 1");
    assert!(frame1_opt.is_some(), "Expected frame 1");
    let frame1 = frame1_opt.unwrap();
    assert_eq!(frame1.width, 1920);
    assert_eq!(frame1.height, 1080);
    assert_eq!(frame1.stride, 1920 * 4);
    assert_eq!(frame1.data.len(), 1920 * 1080 * 4);
    assert!(frame1.timestamp_us > 0);

    // Capture second frame and verify timestamp progression
    let frame2_opt = capturer.capture_frame(50).expect("Frame capture 2");
    assert!(frame2_opt.is_some(), "Expected frame 2");
    let frame2 = frame2_opt.unwrap();
    assert!(
        frame2.timestamp_us >= frame1.timestamp_us,
        "Timestamp should monotonically advance"
    );

    // Stop capturer
    let stop_res = capturer.stop();
    assert!(stop_res.is_ok(), "stop() should succeed");
    assert!(!capturer.is_running());
    assert_eq!(capturer.output_index(), None);

    // Subsequent capture must fail
    assert!(capturer.capture_frame(50).is_err());
}

#[test]
fn test_screen_capturer_simulated_resolution_change() {
    let mut capturer = DxgiCapturer::new_simulated();

    // Set 1440p resolution
    capturer
        .set_simulated_resolution(2560, 1440)
        .expect("Set 2560x1440");
    capturer.start(1).expect("Start output 1");

    let frame = capturer.capture_frame(50).unwrap().unwrap();
    assert_eq!(frame.width, 2560);
    assert_eq!(frame.height, 1440);
    assert_eq!(frame.stride, 2560 * 4);
    assert_eq!(frame.data.len(), 2560 * 1440 * 4);

    // Reject invalid resolution
    assert!(capturer.set_simulated_resolution(0, 1080).is_err());
    assert!(capturer.set_simulated_resolution(1920, 0).is_err());
    assert!(capturer.set_simulated_resolution(10000, 1080).is_err());
}

#[test]
fn test_screen_capturer_timeout_handling() {
    let mut capturer = DxgiCapturer::new_simulated();
    capturer.start(0).unwrap();

    // Normal capture returns a frame
    let normal_frame = capturer.capture_frame(50).unwrap();
    assert!(normal_frame.is_some());

    // Enable timeout simulation
    capturer.set_simulated_timeout(true);
    let timeout_frame = capturer.capture_frame(50).unwrap();
    assert!(
        timeout_frame.is_none(),
        "capture_frame with timeout triggered must return Ok(None)"
    );

    // Disable timeout simulation
    capturer.set_simulated_timeout(false);
    let resumed_frame = capturer.capture_frame(50).unwrap();
    assert!(resumed_frame.is_some());
}

#[test]
fn test_default_new_is_not_simulated() {
    let capturer = DxgiCapturer::new();
    assert!(
        !capturer.is_simulated(),
        "Default DxgiCapturer::new() must NOT default to simulated mode"
    );
}

#[test]
fn test_cursor_compositing_color_alpha_blend() {
    let width = 100u32;
    let height = 100u32;
    let stride = width * 4;
    // Base frame: solid blue (R=0, G=0, B=255, A=255)
    let mut frame_data = vec![0u8; (stride * height) as usize];
    for y in 0..height {
        for x in 0..width {
            let idx = (y * stride + x * 4) as usize;
            frame_data[idx] = 0;       // R
            frame_data[idx + 1] = 0;   // G
            frame_data[idx + 2] = 255; // B
            frame_data[idx + 3] = 255; // A
        }
    }

    // 2x2 color cursor: 50% translucent red (R=255, G=0, B=0, A=128)
    let cursor_shape = PointerShape {
        shape_type: PointerShapeType::ColorRgba,
        width: 2,
        height: 2,
        pitch: 2 * 4,
        hotspot_x: 0,
        hotspot_y: 0,
        data: vec![
            255, 0, 0, 128,  255, 0, 0, 128,
            255, 0, 0, 128,  255, 0, 0, 128,
        ],
    };

    let position = PointerPosition {
        x: 10,
        y: 10,
        visible: true,
    };

    composite_cursor(
        &mut frame_data,
        width,
        height,
        stride,
        &position,
        &cursor_shape,
    )
    .expect("composite_cursor should succeed");

    // Pixel at (10, 10) should be blended:
    // R: (255 * 128 + 0 * 127) / 255 ~= 128
    // G: 0
    // B: (0 * 128 + 255 * 127) / 255 ~= 127
    let idx = (10 * stride + 10 * 4) as usize;
    let r = frame_data[idx];
    let g = frame_data[idx + 1];
    let b = frame_data[idx + 2];
    let a = frame_data[idx + 3];

    assert!((126..=130).contains(&r), "Red should be ~128, got {}", r);
    assert_eq!(g, 0, "Green should be 0");
    assert!((125..=129).contains(&b), "Blue should be ~127, got {}", b);
    assert_eq!(a, 255, "Alpha should remain 255");

    // Pixel at (0, 0) should remain pristine solid blue
    assert_eq!(frame_data[0], 0);
    assert_eq!(frame_data[1], 0);
    assert_eq!(frame_data[2], 255);
    assert_eq!(frame_data[3], 255);
}

#[test]
fn test_cursor_compositing_color_opaque() {
    let width = 50u32;
    let height = 50u32;
    let stride = width * 4;
    // Base frame: solid red
    let mut frame_data = vec![0u8; (stride * height) as usize];
    for y in 0..height {
        for x in 0..width {
            let idx = (y * stride + x * 4) as usize;
            frame_data[idx] = 255;
            frame_data[idx + 1] = 0;
            frame_data[idx + 2] = 0;
            frame_data[idx + 3] = 255;
        }
    }

    // 1x1 fully opaque green cursor
    let cursor_shape = PointerShape {
        shape_type: PointerShapeType::ColorRgba,
        width: 1,
        height: 1,
        pitch: 4,
        hotspot_x: 0,
        hotspot_y: 0,
        data: vec![0, 255, 0, 255],
    };

    let position = PointerPosition {
        x: 5,
        y: 5,
        visible: true,
    };

    composite_cursor(
        &mut frame_data,
        width,
        height,
        stride,
        &position,
        &cursor_shape,
    )
    .expect("composite_cursor");

    let idx = (5 * stride + 5 * 4) as usize;
    assert_eq!(frame_data[idx], 0);
    assert_eq!(frame_data[idx + 1], 255);
    assert_eq!(frame_data[idx + 2], 0);
    assert_eq!(frame_data[idx + 3], 255);
}

#[test]
fn test_cursor_compositing_monochrome_mask() {
    let width = 16u32;
    let height = 16u32;
    let stride = width * 4;
    // Base frame: gray (128, 128, 128, 255)
    let mut frame_data = vec![0u8; (stride * height) as usize];
    for y in 0..height {
        for x in 0..width {
            let idx = (y * stride + x * 4) as usize;
            frame_data[idx] = 128;
            frame_data[idx + 1] = 128;
            frame_data[idx + 2] = 128;
            frame_data[idx + 3] = 255;
        }
    }

    // 2x2 monochrome cursor:
    // Pitch = 1 byte (holds up to 8 pixels)
    // Top 2 rows = AND mask (2 bytes)
    // Bottom 2 rows = XOR mask (2 bytes)
    // Total buffer = 4 bytes
    //
    // Let:
    // Row 0: Pixel (0,0): AND=0, XOR=0 (Black)
    //        Pixel (1,0): AND=0, XOR=1 (White)
    // Row 1: Pixel (0,1): AND=1, XOR=0 (Transparent / Unchanged)
    //        Pixel (1,1): AND=1, XOR=1 (Inverted -> 255 - 128 = 127)
    //
    // AND mask row 0: bits = 0 0 ... = 0x00
    // AND mask row 1: bits = 1 1 ... = 0xC0 (bit 7 and bit 6 set)
    // XOR mask row 0: bits = 0 1 ... = 0x40 (bit 6 set)
    // XOR mask row 1: bits = 0 1 ... = 0x40 (bit 6 set -> inverted on (1,1))
    let and_row0 = 0b0000_0000u8;
    let and_row1 = 0b1100_0000u8;
    let xor_row0 = 0b0100_0000u8;
    let xor_row1 = 0b0100_0000u8;

    let cursor_shape = PointerShape {
        shape_type: PointerShapeType::Monochrome,
        width: 2,
        height: 2,
        pitch: 1,
        hotspot_x: 0,
        hotspot_y: 0,
        data: vec![and_row0, and_row1, xor_row0, xor_row1],
    };

    let position = PointerPosition {
        x: 0,
        y: 0,
        visible: true,
    };

    composite_cursor(
        &mut frame_data,
        width,
        height,
        stride,
        &position,
        &cursor_shape,
    )
    .expect("composite_cursor monochrome");

    // Check (0,0): Black
    let idx_0_0 = 0usize;
    assert_eq!(&frame_data[idx_0_0..idx_0_0 + 4], &[0, 0, 0, 255], "(0,0) must be black");

    // Check (1,0): White
    let idx_1_0 = 4usize;
    assert_eq!(&frame_data[idx_1_0..idx_1_0 + 4], &[255, 255, 255, 255], "(1,0) must be white");

    // Check (0,1): Transparent (unchanged 128, 128, 128)
    let idx_0_1 = (stride) as usize;
    assert_eq!(&frame_data[idx_0_1..idx_0_1 + 4], &[128, 128, 128, 255], "(0,1) must be unchanged");

    // Check (1,1): Inverted (255 - 128 = 127)
    let idx_1_1 = (stride + 4) as usize;
    assert_eq!(&frame_data[idx_1_1..idx_1_1 + 4], &[127, 127, 127, 255], "(1,1) must be inverted");
}

#[test]
fn test_cursor_compositing_invisible_cursor() {
    let width = 20u32;
    let height = 20u32;
    let stride = width * 4;
    let mut frame_data = vec![42u8; (stride * height) as usize];
    let original = frame_data.clone();

    let cursor_shape = PointerShape {
        shape_type: PointerShapeType::ColorRgba,
        width: 4,
        height: 4,
        pitch: 16,
        hotspot_x: 0,
        hotspot_y: 0,
        data: vec![255; 4 * 4 * 4],
    };

    let invisible_pos = PointerPosition {
        x: 5,
        y: 5,
        visible: false, // Cursor hidden
    };

    composite_cursor(
        &mut frame_data,
        width,
        height,
        stride,
        &invisible_pos,
        &cursor_shape,
    )
    .expect("composite_cursor");

    assert_eq!(frame_data, original, "Invisible cursor must not modify frame buffer");
}

#[test]
fn test_cursor_compositing_boundary_clipping() {
    let width = 20u32;
    let height = 20u32;
    let stride = width * 4;
    let mut frame_data = vec![0u8; (stride * height) as usize];

    let cursor_shape = PointerShape {
        shape_type: PointerShapeType::ColorRgba,
        width: 10,
        height: 10,
        pitch: 40,
        hotspot_x: 0,
        hotspot_y: 0,
        data: vec![255; 10 * 10 * 4],
    };

    // Partially off top-left (x = -5, y = -5)
    let pos_top_left = PointerPosition {
        x: -5,
        y: -5,
        visible: true,
    };
    assert!(composite_cursor(
        &mut frame_data,
        width,
        height,
        stride,
        &pos_top_left,
        &cursor_shape,
    )
    .is_ok());

    // Partially off bottom-right (x = 15, y = 15)
    let pos_bottom_right = PointerPosition {
        x: 15,
        y: 15,
        visible: true,
    };
    assert!(composite_cursor(
        &mut frame_data,
        width,
        height,
        stride,
        &pos_bottom_right,
        &cursor_shape,
    )
    .is_ok());

    // Completely off-screen (x = 500, y = 500)
    let pos_off_screen = PointerPosition {
        x: 500,
        y: 500,
        visible: true,
    };
    assert!(composite_cursor(
        &mut frame_data,
        width,
        height,
        stride,
        &pos_off_screen,
        &cursor_shape,
    )
    .is_ok());
}

#[test]
fn test_cursor_compositing_hotspot_offset() {
    let width = 30u32;
    let height = 30u32;
    let stride = width * 4;
    let mut frame_data = vec![0u8; (stride * height) as usize];

    // 4x4 cursor with hotspot at (2, 2)
    let cursor_shape = PointerShape {
        shape_type: PointerShapeType::ColorRgba,
        width: 4,
        height: 4,
        pitch: 16,
        hotspot_x: 2,
        hotspot_y: 2,
        data: vec![200; 4 * 4 * 4],
    };

    // Position at (10, 10). With hotspot (2, 2), top-left corner lands at (8, 8).
    let position = PointerPosition {
        x: 10,
        y: 10,
        visible: true,
    };

    composite_cursor(
        &mut frame_data,
        width,
        height,
        stride,
        &position,
        &cursor_shape,
    )
    .expect("composite_cursor");

    // (8, 8) must have been drawn
    let idx_8_8 = (8 * stride + 8 * 4) as usize;
    assert_eq!(frame_data[idx_8_8], 200);

    // (7, 7) must remain 0
    let idx_7_7 = (7 * stride + 7 * 4) as usize;
    assert_eq!(frame_data[idx_7_7], 0);
}

#[test]
fn test_drop_safety() {
    {
        let mut capturer = DxgiCapturer::new_simulated();
        capturer.start(0).unwrap();
        assert!(capturer.is_running());
        // capturer dropped here; Drop should cleanly stop and release resources
    }
    // Clean exit without panic
}
