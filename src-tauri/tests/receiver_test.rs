//! Unit and integration tests for Native Monitor (Receiver) Rendering Window
//!
//! Tests ClientScreenGeometry, ReceiverConfig, WebRTC Client RTP receiver & RFC 6184
//! H.264 depacketization (Single NALU and FU-A reassembly), sequence gap detection,
//! RTCP PLI feedback generation, presentation surface abstraction with <1 frame latency
//! queue management, and MonitorReceiver connection lifecycle.

use std::sync::Arc;
use tokio::sync::Mutex;
use wireless_monitor_lib::capture::CapturedFrame;
use wireless_monitor_lib::receiver::{
    ClientScreenGeometry, H264Depacketizer, MonitorReceiver, PresentationSurface,
    ReceiverConfig, ReceiverStats, SurfaceMode, SurfaceStats,
};
use wireless_monitor_lib::stream::{
    EncodedPacket, MockVideoEncoder, RtpPacket, RtpPacketizer, StreamConfig, VideoEncoder,
};

#[test]
fn test_client_screen_geometry_defaults_and_validation() {
    let default_geo = ClientScreenGeometry::default();
    assert_eq!(default_geo.width, 1920);
    assert_eq!(default_geo.height, 1080);
    assert_eq!(default_geo.refresh_rate, 60);
    assert!(default_geo.validate().is_ok());

    let custom_geo = ClientScreenGeometry::new(2560, 1440, 120).expect("Valid geometry");
    assert_eq!(custom_geo.width, 2560);
    assert_eq!(custom_geo.height, 1440);
    assert_eq!(custom_geo.refresh_rate, 120);
    let aspect = custom_geo.aspect_ratio();
    assert!((aspect - (2560.0 / 1440.0)).abs() < 1e-4);

    // 4K 60Hz
    let geo_4k = ClientScreenGeometry::new(3840, 2160, 60).expect("Valid 4K geometry");
    assert!(geo_4k.validate().is_ok());

    // Invalid zero width
    assert!(
        ClientScreenGeometry::new(0, 1080, 60).is_err(),
        "Zero width must fail validation"
    );

    // Invalid zero height
    assert!(
        ClientScreenGeometry::new(1920, 0, 60).is_err(),
        "Zero height must fail validation"
    );

    // Invalid zero refresh rate
    assert!(
        ClientScreenGeometry::new(1920, 1080, 0).is_err(),
        "Zero refresh rate must fail validation"
    );

    // Invalid excessive refresh rate
    assert!(
        ClientScreenGeometry::new(1920, 1080, 500).is_err(),
        "Excessive refresh rate (>360) must fail validation"
    );

    // Query primary display fallback check
    let queried = ClientScreenGeometry::query_primary_display();
    assert!(queried.width > 0);
    assert!(queried.height > 0);
    assert!(queried.refresh_rate > 0);
}

#[test]
fn test_receiver_config_serialization() {
    let default_cfg = ReceiverConfig::default();
    assert!(default_cfg.fullscreen);
    assert_eq!(default_cfg.target_display_id, None);

    let custom_cfg = ReceiverConfig {
        fullscreen: false,
        target_display_id: Some("DISPLAY2".to_string()),
    };

    let json = serde_json::to_string(&custom_cfg).expect("Serialization failed");
    assert!(json.contains("fullscreen"));
    assert!(json.contains("false"));
    assert!(json.contains("DISPLAY2"));

    let deserialized: ReceiverConfig =
        serde_json::from_str(&json).expect("Deserialization failed");
    assert_eq!(deserialized.fullscreen, false);
    assert_eq!(
        deserialized.target_display_id,
        Some("DISPLAY2".to_string())
    );
}

#[test]
fn test_rtp_depacketizer_single_nalu_reassembly() {
    let mut depacketizer = H264Depacketizer::new(0x11223344);

    // Create a single NALU packet (non-IDR slice, NAL type 1 -> 0x41)
    let nalu_payload = vec![0x41, 0x9A, 0x01, 0xAA, 0xBB, 0xCC];
    let packet = RtpPacket {
        payload_type: 96,
        sequence_number: 1,
        timestamp: 90_000, // 1.0 second @ 90kHz
        ssrc: 0xAABBCCDD,
        marker: true, // End of frame
        payload: nalu_payload.clone(),
    };

    let result = depacketizer
        .depacketize(&packet)
        .expect("Depacketization must succeed");

    assert!(result.is_some(), "Marker packet must yield a reassembled frame");
    let frame_data = result.unwrap();
    assert_eq!(frame_data.timestamp_us, 1_000_000);
    assert!(
        frame_data.data.starts_with(&[0x00, 0x00, 0x00, 0x01, 0x41]),
        "Reassembled Annex-B must start with 4-byte start code + NALU header"
    );
    assert_eq!(
        frame_data.data.len(),
        4 + nalu_payload.len(),
        "Annex-B length must equal 4 start code bytes + payload"
    );

    assert_eq!(depacketizer.stats().frames_reassembled, 1);
    assert_eq!(depacketizer.stats().packets_received, 1);
    assert_eq!(depacketizer.stats().packets_lost, 0);
}

#[test]
fn test_rtp_depacketizer_fu_a_fragmentation_reassembly() {
    let mut depacketizer = H264Depacketizer::new(0x11223344);
    let mut packetizer = RtpPacketizer::new(1200, 0xAABBCCDD, 96);

    // Construct a large IDR NALU (type 5 -> 0x65, nal_ref_idc 3 -> 0x65)
    let mut big_nalu_data = vec![0x00, 0x00, 0x00, 0x01, 0x65];
    big_nalu_data.extend(vec![0x77u8; 3000]);

    let enc_packet = EncodedPacket {
        is_keyframe: true,
        timestamp_us: 2_000_000,
        data: big_nalu_data,
    };

    let rtp_packets = packetizer.packetize(&enc_packet);
    assert!(
        rtp_packets.len() >= 3,
        "3000-byte NALU with MTU 1200 must generate >= 3 FU-A packets"
    );

    // Process all packets through depacketizer
    let mut assembled_frame = None;
    let total_pkts = rtp_packets.len();

    for (idx, pkt) in rtp_packets.iter().enumerate() {
        let res = depacketizer
            .depacketize(pkt)
            .expect("Depacketization of valid FU-A fragment must succeed");

        if idx < total_pkts - 1 {
            assert!(
                res.is_none(),
                "Non-marker intermediate FU-A fragment {} must return None",
                idx
            );
        } else {
            assembled_frame = res;
        }
    }

    assert!(
        assembled_frame.is_some(),
        "Final FU-A fragment with marker must return reassembled frame"
    );
    let frame = assembled_frame.unwrap();
    assert_eq!(frame.timestamp_us, 2_000_000);
    assert!(frame.is_keyframe);
    assert!(frame.data.starts_with(&[0x00, 0x00, 0x00, 0x01, 0x65]));
    assert_eq!(
        frame.data.len(),
        4 + 1 + 3000,
        "Total reassembled Annex-B size must match 4-byte start code + NALU header + payload"
    );

    assert_eq!(depacketizer.stats().frames_reassembled, 1);
    assert_eq!(depacketizer.stats().fua_fragments_processed, total_pkts as u64);
    assert_eq!(depacketizer.stats().packets_lost, 0);
}

#[test]
fn test_rtp_depacketizer_multi_nalu_keyframe() {
    let mut depacketizer = H264Depacketizer::new(0x11223344);

    // Frame with SPS (type 7), PPS (type 8), and IDR slice (type 5)
    let sps_pkt = RtpPacket {
        payload_type: 96,
        sequence_number: 10,
        timestamp: 180_000, // 2.0s
        ssrc: 0x99999999,
        marker: false, // SPS is not end of frame
        payload: vec![0x67, 0x42, 0xE0, 0x1F],
    };

    let pps_pkt = RtpPacket {
        payload_type: 96,
        sequence_number: 11,
        timestamp: 180_000,
        ssrc: 0x99999999,
        marker: false, // PPS is not end of frame
        payload: vec![0x68, 0xCE, 0x3C, 0x80],
    };

    let idr_pkt = RtpPacket {
        payload_type: 96,
        sequence_number: 12,
        timestamp: 180_000,
        ssrc: 0x99999999,
        marker: true, // End of frame
        payload: vec![0x65, 0x88, 0x84, 0x00, 0xDD, 0xEE],
    };

    assert!(depacketizer.depacketize(&sps_pkt).unwrap().is_none());
    assert!(depacketizer.depacketize(&pps_pkt).unwrap().is_none());

    let final_res = depacketizer.depacketize(&idr_pkt).unwrap();
    assert!(final_res.is_some());

    let frame = final_res.unwrap();
    assert!(frame.is_keyframe);
    assert_eq!(frame.timestamp_us, 2_000_000);

    // Verify all 3 NALUs exist in reassembled Annex-B buffer
    let encoded = EncodedPacket {
        is_keyframe: frame.is_keyframe,
        timestamp_us: frame.timestamp_us,
        data: frame.data,
    };
    assert_eq!(encoded.nalu_count(), 3);
    assert!(encoded.contains_sps_pps());

    let nalus = encoded.extract_nalus();
    assert_eq!(nalus[0][0] & 0x1F, 7); // SPS
    assert_eq!(nalus[1][0] & 0x1F, 8); // PPS
    assert_eq!(nalus[2][0] & 0x1F, 5); // IDR
}

#[test]
fn test_sequence_gap_detection_and_pli_generation() {
    let mut depacketizer = H264Depacketizer::new(0x11223344);

    let pkt1 = RtpPacket {
        payload_type: 96,
        sequence_number: 50,
        timestamp: 90_000,
        ssrc: 0x55555555,
        marker: true,
        payload: vec![0x41, 0x11, 0x22],
    };

    let res1 = depacketizer.depacketize(&pkt1).expect("Packet 50 succeeds");
    assert!(res1.is_some());
    assert_eq!(depacketizer.stats().packets_lost, 0);
    assert_eq!(depacketizer.stats().pli_sent, 0);

    // Send packet 53 (missing 51 and 52 -> sequence gap of 2 packets!)
    let pkt_gap = RtpPacket {
        payload_type: 96,
        sequence_number: 53,
        timestamp: 93_000,
        ssrc: 0x55555555,
        marker: true,
        payload: vec![0x41, 0x33, 0x44],
    };

    let res_gap = depacketizer.depacketize(&pkt_gap).expect("Processed with drop recovery");
    // Frame with gap is discarded or flagged to prevent display artifact
    assert!(res_gap.is_none());

    // Verify sequence gap detection stats
    assert_eq!(depacketizer.stats().packets_lost, 2);
    assert_eq!(depacketizer.stats().pli_sent, 1);

    // Verify RTCP PLI packet was queued
    let rtcp_pkts = depacketizer.take_pending_rtcp();
    assert_eq!(rtcp_pkts.len(), 1, "Must generate exactly 1 RTCP PLI packet");

    let pli = &rtcp_pkts[0];
    assert_eq!(pli.len(), 12, "Standard RFC 4585 PLI packet is 12 bytes");
    // Byte 0: V=2, P=0, FMT=1 -> 0x81
    assert_eq!(pli[0], 0x81, "PLI header byte 0 must be 0x81 (V=2, FMT=1)");
    // Byte 1: PT=206 (PSFB) -> 0xCE
    assert_eq!(pli[1], 206, "PLI payload type must be 206 (PSFB)");
    // Length: 2 (words)
    assert_eq!(u16::from_be_bytes([pli[2], pli[3]]), 2);
    // SSRC of sender (receiver)
    assert_eq!(u32::from_be_bytes([pli[4], pli[5], pli[6], pli[7]]), 0x11223344);
    // SSRC of media source (host)
    assert_eq!(u32::from_be_bytes([pli[8], pli[9], pli[10], pli[11]]), 0x55555555);
}

#[test]
fn test_broken_fu_a_sequence_generates_pli() {
    let mut depacketizer = H264Depacketizer::new(0x11223344);

    // FU-A indicator: type 28 (0x5C)
    // FU-A header: Start bit set (0x80 | 5 = 0x85)
    let pkt_start = RtpPacket {
        payload_type: 96,
        sequence_number: 200,
        timestamp: 100_000,
        ssrc: 0x77777777,
        marker: false,
        payload: vec![0x5C, 0x85, 0xAA, 0xBB],
    };

    assert!(depacketizer.depacketize(&pkt_start).unwrap().is_none());

    // Sequence jump! Skip 201, send 202 as FU-A end (End bit set: 0x40 | 5 = 0x45)
    let pkt_end_broken = RtpPacket {
        payload_type: 96,
        sequence_number: 202,
        timestamp: 100_000,
        ssrc: 0x77777777,
        marker: true,
        payload: vec![0x5C, 0x45, 0xCC, 0xDD],
    };

    let res = depacketizer.depacketize(&pkt_end_broken).unwrap();
    assert!(res.is_none(), "Corrupted FU-A frame must be dropped");

    assert!(depacketizer.stats().packets_lost >= 1);
    assert_eq!(depacketizer.stats().pli_sent, 1);

    let rtcp = depacketizer.take_pending_rtcp();
    assert_eq!(rtcp.len(), 1);
    assert_eq!(rtcp[0][0] & 0x1F, 1); // PLI
    assert_eq!(rtcp[0][1], 206);
}

#[test]
fn test_presentation_surface_simulated_lifecycle() {
    let geo = ClientScreenGeometry::new(1920, 1080, 60).unwrap();
    let config = ReceiverConfig {
        fullscreen: true,
        target_display_id: None,
    };

    let mut surface = PresentationSurface::new_simulated(geo, config.clone());
    assert_eq!(surface.mode(), SurfaceMode::Simulated);
    assert!(surface.is_active());
    assert!(surface.is_fullscreen());
    assert_eq!(surface.geometry().width, 1920);
    assert_eq!(surface.geometry().height, 1080);

    // Queue frames
    let frame1 = CapturedFrame {
        width: 1920,
        height: 1080,
        stride: 1920 * 4,
        data: vec![10u8; 1920 * 1080 * 4],
        timestamp_us: 100_000,
    };
    let frame2 = CapturedFrame {
        width: 1920,
        height: 1080,
        stride: 1920 * 4,
        data: vec![20u8; 1920 * 1080 * 4],
        timestamp_us: 116_666,
    };
    let frame3 = CapturedFrame {
        width: 1920,
        height: 1080,
        stride: 1920 * 4,
        data: vec![30u8; 1920 * 1080 * 4],
        timestamp_us: 133_333,
    };

    // Low-latency queue with capacity 2: pushing 3 frames causes frame 1 to drop
    surface.queue_frame(frame1).expect("Queue frame 1");
    surface.queue_frame(frame2).expect("Queue frame 2");
    surface.queue_frame(frame3).expect("Queue frame 3");

    let stats = surface.stats();
    assert_eq!(stats.dropped_frames, 1, "Oldest frame must drop to preserve <1 frame latency");
    assert_eq!(stats.queued_frames, 2);

    // Render next frame
    let rendered = surface.render_frame().expect("Render frame");
    assert!(rendered.is_some());
    assert_eq!(rendered.unwrap().timestamp_us, 116_666);
    assert_eq!(surface.stats().rendered_frames, 1);

    // Toggle fullscreen
    surface.set_fullscreen(false).expect("Toggle fullscreen");
    assert!(!surface.is_fullscreen());

    // Close surface
    surface.close().expect("Close surface");
    assert!(!surface.is_active());
    assert!(surface.queue_frame(frame2).is_err(), "Queueing to closed surface must Err");
}

#[tokio::test]
async fn test_monitor_receiver_connect_and_handshake() {
    let geo = ClientScreenGeometry::new(1920, 1080, 60).unwrap();

    // Valid connection
    let mut receiver = MonitorReceiver::connect("127.0.0.1", 8080, "1234", geo)
        .await
        .expect("Connect with valid PIN must succeed");

    assert!(receiver.is_connected());
    assert!(!receiver.is_presenting());
    assert_eq!(receiver.geometry().width, 1920);
    assert_eq!(receiver.geometry().height, 1080);

    // Start presentation
    let config = ReceiverConfig {
        fullscreen: true,
        target_display_id: None,
    };
    receiver
        .start_presentation(config)
        .await
        .expect("Start presentation must succeed");
    assert!(receiver.is_presenting());

    // Feed RTP packet
    let packet = RtpPacket {
        payload_type: 96,
        sequence_number: 1,
        timestamp: 90_000,
        ssrc: 0x12345678,
        marker: true,
        payload: vec![0x41, 0x88, 0x99],
    };

    let reassembled = receiver
        .reassemble_rtp_packet(&packet)
        .expect("Reassemble RTP packet");
    assert!(reassembled.is_some());
    let frame = reassembled.unwrap();
    assert_eq!(frame.width, 1920);
    assert_eq!(frame.height, 1080);
    assert!(frame.is_valid());

    // Disconnect
    receiver.disconnect().await.expect("Disconnect must succeed");
    assert!(!receiver.is_connected());
    assert!(!receiver.is_presenting());

    // Idempotent disconnect
    assert!(receiver.disconnect().await.is_ok());
}

#[tokio::test]
async fn test_monitor_receiver_invalid_pin_and_geometry_rejection() {
    let valid_geo = ClientScreenGeometry::new(1920, 1080, 60).unwrap();
    let invalid_geo = ClientScreenGeometry {
        width: 0,
        height: 1080,
        refresh_rate: 60,
    };

    // Invalid PIN length
    assert!(
        MonitorReceiver::connect("127.0.0.1", 8080, "12", valid_geo).await.is_err(),
        "Short PIN must fail"
    );

    // Non-numeric PIN
    assert!(
        MonitorReceiver::connect("127.0.0.1", 8080, "abcd", valid_geo).await.is_err(),
        "Non-numeric PIN must fail"
    );

    // Invalid Geometry
    assert!(
        MonitorReceiver::connect("127.0.0.1", 8080, "1234", invalid_geo).await.is_err(),
        "Invalid geometry must fail"
    );

    // Invalid zero port
    assert!(
        MonitorReceiver::connect("127.0.0.1", 0, "1234", valid_geo).await.is_err(),
        "Zero port must fail"
    );
}

#[tokio::test]
async fn test_monitor_receiver_presentation_queue_latency() {
    let geo = ClientScreenGeometry::new(1920, 1080, 60).unwrap();
    let mut receiver = MonitorReceiver::connect("127.0.0.1", 8080, "9999", geo)
        .await
        .unwrap();

    receiver
        .start_presentation(ReceiverConfig::default())
        .await
        .unwrap();

    // Reassemble multiple packets rapidly into the surface queue
    for i in 1..=5 {
        let pkt = RtpPacket {
            payload_type: 96,
            sequence_number: i,
            timestamp: (i as u32) * 1500,
            ssrc: 0x99999999,
            marker: true,
            payload: vec![0x41, 0x01, 0x02],
        };
        let frame = receiver.reassemble_rtp_packet(&pkt).unwrap();
        assert!(frame.is_some());
    }

    // Verify low-latency queue dropped older frames
    if let Some(surface) = receiver.surface() {
        let stats = surface.stats();
        assert!(
            stats.dropped_frames >= 3,
            "Queue overflow must drop older frames to guarantee <1 frame latency"
        );
        assert!(stats.queued_frames <= 2);
    } else {
        panic!("Surface must be active when presenting");
    }

    receiver.disconnect().await.unwrap();
}

#[test]
fn test_monitor_receiver_thread_safety_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<ClientScreenGeometry>();
    assert_send_sync::<ReceiverConfig>();
    assert_send_sync::<H264Depacketizer>();
    assert_send_sync::<PresentationSurface>();
    assert_send_sync::<MonitorReceiver>();
    assert_send_sync::<ReceiverStats>();
    assert_send_sync::<SurfaceStats>();
}
