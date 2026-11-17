//! Unit and integration tests for H.264 Video Encoding & WebRTC Streaming Engine

use std::sync::Arc;
use wireless_monitor_lib::capture::CapturedFrame;
use wireless_monitor_lib::stream::{
    detect_hardware_encoder, EncodedPacket, EncoderBackend, MockVideoEncoder,
    RtpPacket, RtpPacketizer, SessionState, StreamConfig, VideoEncoder, WebRtcHostSession,
};

#[test]
fn test_stream_config_validation_and_defaults() {
    let default_cfg = StreamConfig::default();
    assert_eq!(default_cfg.target_bitrate_kbps, 10_000);
    assert_eq!(default_cfg.max_framerate, 60);
    assert!(default_cfg.validate().is_ok());

    let custom_cfg = StreamConfig {
        target_bitrate_kbps: 25_000,
        max_framerate: 120,
    };
    assert!(custom_cfg.validate().is_ok());

    let invalid_zero_bitrate = StreamConfig {
        target_bitrate_kbps: 0,
        max_framerate: 60,
    };
    assert!(
        invalid_zero_bitrate.validate().is_err(),
        "Zero bitrate must fail validation"
    );

    let invalid_excessive_bitrate = StreamConfig {
        target_bitrate_kbps: 500_000, // Exceeds 200 Mbps cap
        max_framerate: 60,
    };
    assert!(
        invalid_excessive_bitrate.validate().is_err(),
        "Excessive bitrate must fail validation"
    );

    let invalid_zero_fps = StreamConfig {
        target_bitrate_kbps: 5_000,
        max_framerate: 0,
    };
    assert!(
        invalid_zero_fps.validate().is_err(),
        "Zero framerate must fail validation"
    );

    let invalid_excessive_fps = StreamConfig {
        target_bitrate_kbps: 5_000,
        max_framerate: 300, // Exceeds 240 fps cap
    };
    assert!(
        invalid_excessive_fps.validate().is_err(),
        "Excessive framerate must fail validation"
    );
}

#[test]
fn test_encoded_packet_annex_b_structure() {
    // Construct sample Annex-B NALUs: SPS (0x67), PPS (0x68), IDR (0x65)
    let mut annex_b_data = Vec::new();
    // SPS: Start code 00 00 00 01, NALU 67 42 e0 1f
    annex_b_data.extend_from_slice(&[0x00, 0x00, 0x00, 0x01, 0x67, 0x42, 0xe0, 0x1f]);
    // PPS: Start code 00 00 00 01, NALU 68 ce 3c 80
    annex_b_data.extend_from_slice(&[0x00, 0x00, 0x00, 0x01, 0x68, 0xce, 0x3c, 0x80]);
    // IDR: Start code 00 00 00 01, NALU 65 88 84 00
    annex_b_data.extend_from_slice(&[0x00, 0x00, 0x00, 0x01, 0x65, 0x88, 0x84, 0x00]);

    let packet = EncodedPacket {
        is_keyframe: true,
        timestamp_us: 1_000_000,
        data: annex_b_data,
    };

    assert!(packet.is_keyframe);
    assert_eq!(packet.timestamp_us, 1_000_000);
    assert_eq!(packet.nalu_count(), 3);
    assert!(packet.contains_sps_pps());

    let nalus = packet.extract_nalus();
    assert_eq!(nalus.len(), 3);
    assert_eq!(nalus[0][0] & 0x1F, 7, "First NALU must be SPS (type 7)");
    assert_eq!(nalus[1][0] & 0x1F, 8, "Second NALU must be PPS (type 8)");
    assert_eq!(nalus[2][0] & 0x1F, 5, "Third NALU must be IDR (type 5)");
}

#[test]
fn test_mock_video_encoder_lifecycle_and_keyframe() {
    let config = StreamConfig {
        target_bitrate_kbps: 8_000,
        max_framerate: 60,
    };
    let mut encoder = MockVideoEncoder::new(config.clone());
    assert_eq!(encoder.backend_type(), EncoderBackend::Mock);
    assert_eq!(encoder.config().target_bitrate_kbps, 8_000);

    let frame = CapturedFrame {
        width: 1920,
        height: 1080,
        stride: 1920 * 4,
        data: vec![128u8; 1920 * 1080 * 4],
        timestamp_us: 100_000,
    };

    // First frame must be a keyframe (IDR + SPS + PPS)
    let pkts1 = encoder.encode(&frame).expect("First frame encode must succeed");
    assert_eq!(pkts1.len(), 1);
    assert!(pkts1[0].is_keyframe, "First frame MUST be a keyframe");
    assert!(pkts1[0].contains_sps_pps(), "First frame must contain SPS and PPS");
    assert_eq!(pkts1[0].timestamp_us, 100_000);

    // Second frame without request must be an inter-frame (P-frame)
    let frame2 = CapturedFrame {
        width: 1920,
        height: 1080,
        stride: 1920 * 4,
        data: vec![128u8; 1920 * 1080 * 4],
        timestamp_us: 116_666,
    };
    let pkts2 = encoder.encode(&frame2).expect("Second frame encode must succeed");
    assert_eq!(pkts2.len(), 1);
    assert!(!pkts2[0].is_keyframe, "Second frame should be an inter-frame (P-frame)");
    assert!(!pkts2[0].contains_sps_pps(), "Inter-frame should not contain SPS/PPS");

    // Request keyframe explicitly (e.g. from PLI / FIR feedback)
    encoder.request_keyframe();

    let frame3 = CapturedFrame {
        width: 1920,
        height: 1080,
        stride: 1920 * 4,
        data: vec![128u8; 1920 * 1080 * 4],
        timestamp_us: 133_333,
    };
    let pkts3 = encoder.encode(&frame3).expect("Third frame encode must succeed");
    assert_eq!(pkts3.len(), 1);
    assert!(pkts3[0].is_keyframe, "Frame after request_keyframe must be a keyframe");
    assert!(pkts3[0].contains_sps_pps());

    // Encoding invalid frame must fail
    let invalid_frame = CapturedFrame {
        width: 0,
        height: 1080,
        stride: 0,
        data: vec![],
        timestamp_us: 150_000,
    };
    assert!(
        encoder.encode(&invalid_frame).is_err(),
        "Encoding invalid frame must return Err"
    );
}

#[test]
fn test_encoder_hardware_detection_and_fallback() {
    let backend = detect_hardware_encoder();
    // On Windows, MediaFoundation should be detected or software fallback
    #[cfg(windows)]
    assert!(
        backend == EncoderBackend::MediaFoundation || backend == EncoderBackend::SoftwareOpenH264
    );

    #[cfg(not(windows))]
    assert!(backend == EncoderBackend::Vaapi || backend == EncoderBackend::SoftwareOpenH264);
}

#[test]
fn test_rtp_packetizer_single_nalu() {
    let mut packetizer = RtpPacketizer::new(1200, 0x12345678, 96);

    // Single small NALU (e.g. 50 bytes)
    let mut data = vec![0x00, 0x00, 0x00, 0x01];
    data.push(0x41); // Non-IDR slice
    data.extend(vec![0xAA; 45]);

    let packet = EncodedPacket {
        is_keyframe: false,
        timestamp_us: 1_000_000, // 1 sec = 90,000 RTP timestamp ticks
        data,
    };

    let rtp_packets = packetizer.packetize(&packet);
    assert_eq!(rtp_packets.len(), 1, "Small NALU should produce exactly 1 RTP packet");

    let rtp = &rtp_packets[0];
    assert_eq!(rtp.payload_type, 96);
    assert_eq!(rtp.ssrc, 0x12345678);
    assert_eq!(rtp.timestamp, 90_000);
    assert!(rtp.marker, "Single NALU packet at frame end must have marker bit set");
    assert_eq!(rtp.payload[0], 0x41, "Payload must start with NALU header byte");

    // Serialize to bytes and check standard 12-byte RTP header
    let raw_bytes = rtp.to_bytes();
    assert_eq!(raw_bytes.len(), 12 + rtp.payload.len());
    assert_eq!(raw_bytes[0], 0x80, "RTP version 2");
    assert_eq!(raw_bytes[1] & 0x7F, 96, "Payload type 96");
    assert_ne!(raw_bytes[1] & 0x80, 0, "Marker bit must be set in raw byte 1");

    // Roundtrip parse check
    let parsed = RtpPacket::parse(&raw_bytes).expect("Roundtrip RTP parse must succeed");
    assert_eq!(parsed.payload_type, rtp.payload_type);
    assert_eq!(parsed.sequence_number, rtp.sequence_number);
    assert_eq!(parsed.timestamp, rtp.timestamp);
    assert_eq!(parsed.ssrc, rtp.ssrc);
    assert_eq!(parsed.marker, rtp.marker);
    assert_eq!(parsed.payload, rtp.payload);
}

#[test]
fn test_rtp_packetizer_fragmentation_fu_a() {
    let mut packetizer = RtpPacketizer::new(1200, 0xABCDEF01, 96);

    // NALU larger than MTU (e.g. 2500 bytes)
    let mut data = vec![0x00, 0x00, 0x00, 0x01];
    data.push(0x65); // IDR slice (type 5)
    data.extend(vec![0xBB; 2500]);

    let packet = EncodedPacket {
        is_keyframe: true,
        timestamp_us: 2_000_000, // 2 sec = 180,000 RTP ticks
        data,
    };

    let rtp_packets = packetizer.packetize(&packet);
    assert!(
        rtp_packets.len() >= 3,
        "2500-byte NALU with 1200 MTU must be split into >= 3 FU-A packets"
    );

    // Verify FU-A headers
    for (i, rtp) in rtp_packets.iter().enumerate() {
        assert_eq!(rtp.payload_type, 96);
        assert_eq!(rtp.timestamp, 180_000);
        assert_eq!(rtp.ssrc, 0xABCDEF01);

        let fu_indicator = rtp.payload[0];
        let fu_header = rtp.payload[1];

        // FU indicator type must be 28 (FU-A)
        assert_eq!(
            fu_indicator & 0x1F,
            28,
            "FU indicator must specify type 28 (FU-A)"
        );
        // Original NALU type must be preserved in FU header
        assert_eq!(
            fu_header & 0x1F,
            5,
            "Original NALU type 5 must be in FU header"
        );

        if i == 0 {
            // First fragment: Start bit set (0x80), End bit cleared
            assert_ne!(fu_header & 0x80, 0, "First fragment must have Start bit set");
            assert_eq!(fu_header & 0x40, 0, "First fragment must have End bit cleared");
            assert!(!rtp.marker, "First fragment must not have RTP marker set");
        } else if i == rtp_packets.len() - 1 {
            // Last fragment: Start bit cleared, End bit set (0x40)
            assert_eq!(fu_header & 0x80, 0, "Last fragment must have Start bit cleared");
            assert_ne!(fu_header & 0x40, 0, "Last fragment must have End bit set");
            assert!(rtp.marker, "Final fragment must have RTP marker set");
        } else {
            // Middle fragment
            assert_eq!(fu_header & 0x80, 0, "Middle fragment Start bit must be 0");
            assert_eq!(fu_header & 0x40, 0, "Middle fragment End bit must be 0");
            assert!(!rtp.marker, "Middle fragment marker must be 0");
        }
    }

    // Sequence numbers must strictly increment
    for i in 1..rtp_packets.len() {
        assert_eq!(
            rtp_packets[i].sequence_number,
            rtp_packets[i - 1].sequence_number.wrapping_add(1)
        );
    }
}

#[tokio::test]
async fn test_webrtc_host_session_lifecycle() {
    let config = StreamConfig {
        target_bitrate_kbps: 10_000,
        max_framerate: 60,
    };

    let session = WebRtcHostSession::init(config).await.expect("Session init must succeed");
    assert_eq!(session.state(), SessionState::New);
    assert!(!session.is_closed());
    assert!(!session.is_connected());

    // Creating offer
    let offer_sdp = session.create_offer().await.expect("Create offer must succeed");
    assert!(offer_sdp.contains("v=0"), "SDP must contain v=0");
    assert!(offer_sdp.contains("m=video"), "SDP must contain m=video");
    assert!(
        offer_sdp.contains("a=rtpmap:96 H264/90000"),
        "SDP must specify H264/90000"
    );
    assert!(offer_sdp.contains("a=rtcp-fb:96 nack pli"), "SDP must support PLI");
    assert_eq!(session.state(), SessionState::OfferCreated);

    // Apply valid answer SDP
    let answer_sdp = format!(
        "v=0\r\n\
         o=- 123456 2 IN IP4 127.0.0.1\r\n\
         s=-\r\n\
         t=0 0\r\n\
         m=video 9 UDP/TLS/RTP/SAVPF 96\r\n\
         c=IN IP4 127.0.0.1\r\n\
         a=rtpmap:96 H264/90000\r\n\
         a=recvonly\r\n"
    );

    session.apply_answer(&answer_sdp).await.expect("Apply answer must succeed");
    assert_eq!(session.state(), SessionState::Connected);
    assert!(session.is_connected());

    // Close session
    session.close().await.expect("Close session must succeed");
    assert_eq!(session.state(), SessionState::Closed);
    assert!(session.is_closed());
    assert!(!session.is_connected());

    // Calling operations on closed session must fail
    assert!(session.create_offer().await.is_err(), "create_offer on closed session must fail");
    let dummy_frame = CapturedFrame {
        width: 1920,
        height: 1080,
        stride: 1920 * 4,
        data: vec![0u8; 1920 * 1080 * 4],
        timestamp_us: 100_000,
    };
    assert!(session.push_frame(dummy_frame).await.is_err(), "push_frame on closed session must fail");
}

#[tokio::test]
async fn test_webrtc_host_session_invalid_answer_rejection() {
    let session = WebRtcHostSession::init(StreamConfig::default())
        .await
        .expect("Session init must succeed");

    session.create_offer().await.expect("Create offer must succeed");

    // Invalid answer: empty SDP
    assert!(
        session.apply_answer("").await.is_err(),
        "Empty answer SDP must be rejected"
    );

    // Invalid answer: missing video media section
    let audio_only_sdp = "v=0\r\no=- 1 1 IN IP4 127.0.0.1\r\ns=-\r\nt=0 0\r\nm=audio 9 UDP 111\r\n";
    assert!(
        session.apply_answer(audio_only_sdp).await.is_err(),
        "SDP without video media must be rejected"
    );

    // Invalid answer: unsupported codec (e.g. VP8 only)
    let vp8_only_sdp = "v=0\r\no=- 1 1 IN IP4 127.0.0.1\r\ns=-\r\nt=0 0\r\nm=video 9 UDP 97\r\na=rtpmap:97 VP8/90000\r\n";
    assert!(
        session.apply_answer(vp8_only_sdp).await.is_err(),
        "SDP without H264 must be rejected"
    );

    // State must remain OfferCreated after failed attempts
    assert_eq!(session.state(), SessionState::OfferCreated);
}

#[tokio::test]
async fn test_webrtc_host_session_push_frame_pipeline() {
    let session = WebRtcHostSession::init(StreamConfig::default())
        .await
        .expect("Session init must succeed");

    let dummy_frame = CapturedFrame {
        width: 1920,
        height: 1080,
        stride: 1920 * 4,
        data: vec![200u8; 1920 * 1080 * 4],
        timestamp_us: 500_000,
    };

    // Pushing frame before connection established must fail
    assert!(
        session.push_frame(dummy_frame.clone()).await.is_err(),
        "push_frame before answer is applied must fail"
    );

    session.create_offer().await.unwrap();
    let answer_sdp = "v=0\r\no=- 1 1 IN IP4 127.0.0.1\r\ns=-\r\nt=0 0\r\nm=video 9 UDP 96\r\na=rtpmap:96 H264/90000\r\n";
    session.apply_answer(answer_sdp).await.unwrap();

    // Push valid frame
    session.push_frame(dummy_frame).await.expect("push_frame must succeed when connected");

    let packets = session.take_pending_packets();
    assert!(!packets.is_empty(), "Pending RTP packets should be buffered");
    assert_eq!(packets[0].timestamp, 45_000, "500_000 us = 45_000 ticks at 90kHz");
}

#[tokio::test]
async fn test_webrtc_host_session_pli_fir_keyframe_trigger() {
    let session = WebRtcHostSession::init(StreamConfig::default())
        .await
        .expect("Session init must succeed");

    session.create_offer().await.unwrap();
    let answer_sdp = "v=0\r\no=- 1 1 IN IP4 127.0.0.1\r\ns=-\r\nt=0 0\r\nm=video 9 UDP 96\r\na=rtpmap:96 H264/90000\r\n";
    session.apply_answer(answer_sdp).await.unwrap();

    // Frame 1: Initial keyframe
    let f1 = CapturedFrame {
        width: 1920,
        height: 1080,
        stride: 1920 * 4,
        data: vec![100u8; 1920 * 1080 * 4],
        timestamp_us: 10_000,
    };
    session.push_frame(f1).await.unwrap();
    session.take_pending_packets();

    // Frame 2: Inter-frame
    let f2 = CapturedFrame {
        width: 1920,
        height: 1080,
        stride: 1920 * 4,
        data: vec![100u8; 1920 * 1080 * 4],
        timestamp_us: 26_666,
    };
    session.push_frame(f2).await.unwrap();
    session.take_pending_packets();

    // Simulate incoming RTCP PLI packet (PT=206, FMT=1)
    let pli_rtcp = vec![
        0x81, 206, 0x00, 0x02, // V=2, P=0, FMT=1 (PLI), PT=206, Length=2
        0x12, 0x34, 0x56, 0x78, // SSRC of packet sender
        0x00, 0x00, 0x00, 0x01, // SSRC of media source
    ];
    let triggered = session.handle_rtcp_packet(&pli_rtcp).await.unwrap();
    assert!(triggered, "RTCP PLI should trigger keyframe request");

    // Frame 3: Must now be forced into a keyframe
    let f3 = CapturedFrame {
        width: 1920,
        height: 1080,
        stride: 1920 * 4,
        data: vec![100u8; 1920 * 1080 * 4],
        timestamp_us: 43_333,
    };
    session.push_frame(f3).await.unwrap();
    let packets = session.take_pending_packets();
    assert!(!packets.is_empty());
    assert_eq!(session.stats().keyframes_generated, 2, "Should have generated 2 keyframes");
}

#[test]
fn test_webrtc_host_session_thread_safety_send_sync() {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    assert_send::<WebRtcHostSession>();
    assert_sync::<WebRtcHostSession>();
    assert_send::<MockVideoEncoder>();
    assert_sync::<MockVideoEncoder>();
    assert_send::<Box<dyn VideoEncoder>>();
    assert_sync::<Box<dyn VideoEncoder>>();
    assert_send::<RtpPacket>();
    assert_sync::<RtpPacket>();
}
