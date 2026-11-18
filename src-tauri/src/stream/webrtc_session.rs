//! WebRTC Host Peer Connection Session & RTP H.264 Streaming Pipeline

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::capture::CapturedFrame;
use crate::stream::encoder::{create_encoder, VideoEncoder};
use crate::stream::{EncodedPacket, StreamConfig};

// ----------------------------------------------------------------------------
// RTP Packet & H.264 RFC 6184 Packetizer
// ----------------------------------------------------------------------------

/// Standard RFC 3550 RTP Packet structure
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RtpPacket {
    /// Payload type (e.g. 96 for dynamic H.264)
    pub payload_type: u8,
    /// 16-bit sequence number incrementing per packet
    pub sequence_number: u16,
    /// 32-bit timestamp derived from 90 kHz video clock
    pub timestamp: u32,
    /// Synchronization Source identifier
    pub ssrc: u32,
    /// Marker bit (set to 1 on the last packet of a video frame)
    pub marker: bool,
    /// Raw payload bytes (NALU or FU-A fragment)
    pub payload: Vec<u8>,
}

impl RtpPacket {
    /// Serialize the RTP packet into raw wire bytes (12-byte header + payload)
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(12 + self.payload.len());
        // Byte 0: V=2, P=0, X=0, CC=0 -> 0x80
        bytes.push(0x80);
        // Byte 1: M (1 bit) | PT (7 bits)
        let m_pt = (if self.marker { 0x80 } else { 0x00 }) | (self.payload_type & 0x7F);
        bytes.push(m_pt);
        // Bytes 2-3: Sequence number (big endian)
        bytes.extend_from_slice(&self.sequence_number.to_be_bytes());
        // Bytes 4-7: Timestamp (big endian)
        bytes.extend_from_slice(&self.timestamp.to_be_bytes());
        // Bytes 8-11: SSRC (big endian)
        bytes.extend_from_slice(&self.ssrc.to_be_bytes());
        // Payload
        bytes.extend_from_slice(&self.payload);
        bytes
    }

    /// Parse raw bytes into an RTP packet
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < 12 {
            bail!("RTP packet buffer too small: {} bytes (minimum 12)", data.len());
        }

        let b0 = data[0];
        let version = (b0 >> 6) & 0x03;
        if version != 2 {
            bail!("Unsupported RTP version: {}", version);
        }

        let b1 = data[1];
        let marker = (b1 & 0x80) != 0;
        let payload_type = b1 & 0x7F;

        let sequence_number = u16::from_be_bytes([data[2], data[3]]);
        let timestamp = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        let ssrc = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);

        let payload = data[12..].to_vec();

        Ok(Self {
            payload_type,
            sequence_number,
            timestamp,
            ssrc,
            marker,
            payload,
        })
    }
}

/// RTP Packetizer implementing RFC 6184 for H.264
#[derive(Debug, Clone)]
pub struct RtpPacketizer {
    /// Maximum Transmission Unit in bytes (typically 1200)
    pub mtu: usize,
    /// Synchronization Source ID
    pub ssrc: u32,
    /// H.264 payload type (typically 96)
    pub payload_type: u8,
    /// Current sequence number
    pub sequence_number: u16,
    /// Clock rate in Hz (standard 90,000 Hz for video)
    pub clock_rate: u32,
}

impl RtpPacketizer {
    pub fn new(mtu: usize, ssrc: u32, payload_type: u8) -> Self {
        Self {
            mtu: mtu.max(500),
            ssrc,
            payload_type,
            sequence_number: 1,
            clock_rate: 90_000,
        }
    }

    /// Packetize an EncodedPacket containing Annex-B NALUs into RTP packets
    pub fn packetize(&mut self, packet: &EncodedPacket) -> Vec<RtpPacket> {
        let nalus = packet.extract_nalus();
        if nalus.is_empty() {
            return Vec::new();
        }

        let mut rtp_packets = Vec::new();
        let rtp_timestamp =
            ((packet.timestamp_us as u128 * self.clock_rate as u128) / 1_000_000) as u32;

        let max_payload_len = self.mtu.saturating_sub(12); // Subtract 12-byte RTP header
        let total_nalus = nalus.len();

        for (nalu_idx, nalu) in nalus.iter().enumerate() {
            if nalu.is_empty() {
                continue;
            }

            let is_last_nalu = nalu_idx == total_nalus - 1;

            if nalu.len() <= max_payload_len {
                // Single NAL unit packet
                let rtp = RtpPacket {
                    payload_type: self.payload_type,
                    sequence_number: self.sequence_number,
                    timestamp: rtp_timestamp,
                    ssrc: self.ssrc,
                    marker: is_last_nalu,
                    payload: nalu.to_vec(),
                };
                self.sequence_number = self.sequence_number.wrapping_add(1);
                rtp_packets.push(rtp);
            } else {
                // Fragmentation Unit A (FU-A) packetization (RFC 6184 Section 5.8)
                let nalu_header = nalu[0];
                let nalu_type = nalu_header & 0x1F;
                let nalu_ref_idc = nalu_header & 0x60;
                let fu_indicator = nalu_ref_idc | 28; // Type 28 = FU-A

                let nalu_data = &nalu[1..];
                let fu_max_payload = max_payload_len.saturating_sub(2); // 1 byte indicator + 1 byte header
                let total_len = nalu_data.len();
                let mut offset = 0;

                while offset < total_len {
                    let chunk_len = (total_len - offset).min(fu_max_payload);
                    let chunk = &nalu_data[offset..offset + chunk_len];

                    let is_start = offset == 0;
                    let is_end = offset + chunk_len == total_len;

                    let mut fu_header = nalu_type;
                    if is_start {
                        fu_header |= 0x80; // Start bit
                    }
                    if is_end {
                        fu_header |= 0x40; // End bit
                    }

                    let marker = is_last_nalu && is_end;

                    let mut payload = Vec::with_capacity(2 + chunk.len());
                    payload.push(fu_indicator);
                    payload.push(fu_header);
                    payload.extend_from_slice(chunk);

                    let rtp = RtpPacket {
                        payload_type: self.payload_type,
                        sequence_number: self.sequence_number,
                        timestamp: rtp_timestamp,
                        ssrc: self.ssrc,
                        marker,
                        payload,
                    };

                    self.sequence_number = self.sequence_number.wrapping_add(1);
                    rtp_packets.push(rtp);

                    offset += chunk_len;
                }
            }
        }

        rtp_packets
    }
}

// ----------------------------------------------------------------------------
// Session State & Statistics
// ----------------------------------------------------------------------------

/// WebRTC peer connection session state machine
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionState {
    /// Initialized, ready to create offer
    New,
    /// SDP offer created, awaiting receiver SDP answer
    OfferCreated,
    /// SDP answer applied, actively streaming frames
    Connected,
    /// Session terminated and resources closed
    Closed,
}

/// Real-time streaming metrics and statistics
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SessionStats {
    pub frames_encoded: u64,
    pub keyframes_generated: u64,
    pub packets_sent: u64,
    pub bytes_sent: u64,
    pub keyframe_requests_received: u64,
}

// ----------------------------------------------------------------------------
// WebRTC Host Session
// ----------------------------------------------------------------------------

/// WebRTC host peer connection streaming session
pub struct WebRtcHostSession {
    config: StreamConfig,
    state: Arc<Mutex<SessionState>>,
    encoder: Arc<Mutex<Box<dyn VideoEncoder>>>,
    packetizer: Arc<Mutex<RtpPacketizer>>,
    pending_rtp_packets: Arc<Mutex<Vec<RtpPacket>>>,
    local_sdp: Arc<Mutex<Option<String>>>,
    remote_sdp: Arc<Mutex<Option<String>>>,
    is_closed: Arc<AtomicBool>,
    stats: Arc<Mutex<SessionStats>>,
    ice_ufrag: String,
    ice_pwd: String,
}

impl WebRtcHostSession {
    /// Initialize a new WebRTC host session with the provided configuration
    pub async fn init(config: StreamConfig) -> Result<Self> {
        config.validate()?;

        // Attempt to create preferred encoder (hardware accelerated with software/mock fallback)
        let encoder = match create_encoder(config.clone(), true) {
            Ok(enc) => enc,
            Err(_) => Box::new(crate::stream::encoder::MockVideoEncoder::new(config.clone())),
        };

        Self::init_with_encoder(config, encoder)
    }

    /// Initialize a session injecting a specific video encoder implementation
    pub fn init_with_encoder(
        config: StreamConfig,
        encoder: Box<dyn VideoEncoder>,
    ) -> Result<Self> {
        config.validate()?;

        let ssrc = 0x1A2B3C4D;
        let packetizer = RtpPacketizer::new(1200, ssrc, 96);

        Ok(Self {
            config,
            state: Arc::new(Mutex::new(SessionState::New)),
            encoder: Arc::new(Mutex::new(encoder)),
            packetizer: Arc::new(Mutex::new(packetizer)),
            pending_rtp_packets: Arc::new(Mutex::new(Vec::new())),
            local_sdp: Arc::new(Mutex::new(None)),
            remote_sdp: Arc::new(Mutex::new(None)),
            is_closed: Arc::new(AtomicBool::new(false)),
            stats: Arc::new(Mutex::new(SessionStats::default())),
            ice_ufrag: "wmhostufrag".to_string(),
            ice_pwd: "wmhosticepwd9876543210".to_string(),
        })
    }

    /// Dynamically update video encoder bitrate during active session streaming
    pub fn update_bitrate(&self, bitrate_kbps: u32) -> Result<()> {
        if let Ok(mut enc) = self.encoder.try_lock() {
            enc.update_bitrate(bitrate_kbps)?;
        }
        Ok(())
    }

    /// Check whether the session has been closed
    pub fn is_closed(&self) -> bool {
        self.is_closed.load(Ordering::SeqCst)
    }

    /// Check whether the peer connection is actively connected
    pub fn is_connected(&self) -> bool {
        if self.is_closed() {
            return false;
        }
        // Check state synchronously via blocking or lock
        // Note: we can use try_lock or state method
        if let Ok(guard) = self.state.try_lock() {
            *guard == SessionState::Connected
        } else {
            false
        }
    }

    /// Query the current session state
    pub fn state(&self) -> SessionState {
        if self.is_closed() {
            return SessionState::Closed;
        }
        if let Ok(guard) = self.state.try_lock() {
            *guard
        } else {
            SessionState::New
        }
    }

    /// Query current session statistics
    pub fn stats(&self) -> SessionStats {
        if let Ok(guard) = self.stats.try_lock() {
            guard.clone()
        } else {
            SessionStats::default()
        }
    }

    /// Drain all queued RTP packets for transmission
    pub fn take_pending_packets(&self) -> Vec<RtpPacket> {
        if let Ok(mut guard) = self.pending_rtp_packets.try_lock() {
            let packets = guard.clone();
            guard.clear();
            packets
        } else {
            Vec::new()
        }
    }

    /// Generate an RFC 8866 compliant SDP offer describing the H.264 video track
    pub async fn create_offer(&self) -> Result<String> {
        if self.is_closed() {
            bail!("Cannot create offer: WebRTC session is closed");
        }

        let mut state_guard = self.state.lock().await;

        let sdp = format!(
            "v=0\r\n\
             o=- 194820194 2 IN IP4 127.0.0.1\r\n\
             s=WirelessVirtualMonitor\r\n\
             t=0 0\r\n\
             a=group:BUNDLE 0\r\n\
             a=msid-semantic: WMS\r\n\
             m=video 9 UDP/TLS/RTP/SAVPF 96\r\n\
             c=IN IP4 0.0.0.0\r\n\
             a=mid:0\r\n\
             a=sendonly\r\n\
             a=rtpmap:96 H264/90000\r\n\
             a=rtcp-fb:96 nack\r\n\
             a=rtcp-fb:96 nack pli\r\n\
             a=rtcp-fb:96 ccm fir\r\n\
             a=fmtp:96 level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=42e01f\r\n\
             a=ice-ufrag:{}\r\n\
             a=ice-pwd:{}\r\n\
             a=setup:actpass\r\n",
            self.ice_ufrag, self.ice_pwd
        );

        *state_guard = SessionState::OfferCreated;
        let mut local_guard = self.local_sdp.lock().await;
        *local_guard = Some(sdp.clone());

        Ok(sdp)
    }

    /// Process and validate the SDP answer from the receiver peer
    pub async fn apply_answer(&self, sdp: &str) -> Result<()> {
        if self.is_closed() {
            bail!("Cannot apply answer: WebRTC session is closed");
        }

        let mut state_guard = self.state.lock().await;
        if *state_guard != SessionState::OfferCreated {
            bail!(
                "Cannot apply answer in state {:?}: expected OfferCreated",
                *state_guard
            );
        }

        let trimmed = sdp.trim();
        if trimmed.is_empty() {
            bail!("SDP answer cannot be empty");
        }

        if !trimmed.contains("m=video") {
            bail!("SDP answer missing required video media description ('m=video')");
        }

        if !trimmed.contains("H264") && !trimmed.contains("96") {
            bail!("SDP answer does not negotiate compatible H.264 payload type (96)");
        }

        *state_guard = SessionState::Connected;
        let mut remote_guard = self.remote_sdp.lock().await;
        *remote_guard = Some(sdp.to_string());

        Ok(())
    }

    /// Ingest a captured frame, encode via H.264, packetize to RTP, and buffer
    pub async fn push_frame(&self, frame: CapturedFrame) -> Result<()> {
        if self.is_closed() {
            bail!("Cannot push frame: WebRTC session is closed");
        }

        let state_guard = self.state.lock().await;
        if *state_guard != SessionState::Connected {
            bail!("Cannot push frame: WebRTC session is not connected");
        }
        drop(state_guard);

        if !frame.is_valid() {
            bail!("Cannot push invalid CapturedFrame into stream pipeline");
        }

        let mut enc_guard = self.encoder.lock().await;
        let encoded_packets = enc_guard.encode(&frame)?;
        drop(enc_guard);

        let mut pkt_guard = self.packetizer.lock().await;
        let mut pending_guard = self.pending_rtp_packets.lock().await;
        let mut stats_guard = self.stats.lock().await;

        for pkt in encoded_packets {
            stats_guard.frames_encoded += 1;
            if pkt.is_keyframe {
                stats_guard.keyframes_generated += 1;
            }

            let rtp_packets = pkt_guard.packetize(&pkt);
            for rtp in rtp_packets {
                stats_guard.packets_sent += 1;
                stats_guard.bytes_sent += rtp.payload.len() as u64 + 12;
                pending_guard.push(rtp);
            }
        }

        Ok(())
    }

    /// Ingest a captured frame alias for push_frame
    pub async fn ingest_frame(&self, frame: CapturedFrame) -> Result<()> {
        self.push_frame(frame).await
    }

    /// Request an immediate IDR keyframe from the video encoder
    pub async fn request_keyframe(&self) -> Result<()> {
        if self.is_closed() {
            bail!("Cannot request keyframe: WebRTC session is closed");
        }

        let mut enc_guard = self.encoder.lock().await;
        enc_guard.request_keyframe();

        let mut stats_guard = self.stats.lock().await;
        stats_guard.keyframe_requests_received += 1;

        Ok(())
    }

    /// Inspect incoming RTCP feedback packets (e.g. PLI or FIR) and trigger keyframe re-request
    pub async fn handle_rtcp_packet(&self, packet: &[u8]) -> Result<bool> {
        if self.is_closed() || packet.len() < 4 {
            return Ok(false);
        }

        let pt = packet[1];
        let fmt = packet[0] & 0x1F;

        // PT 206 = PSFB (Payload-Specific Feedback)
        // FMT 1 = PLI (Picture Loss Indication)
        // FMT 4 = FIR (Full Intra Request)
        if pt == 206 && (fmt == 1 || fmt == 4) {
            self.request_keyframe().await?;
            return Ok(true);
        }

        Ok(false)
    }

    /// Inspect incoming RTCP feedback packets alias for handle_rtcp_packet
    pub async fn handle_rtcp_feedback(&self, packet: &[u8]) -> Result<bool> {
        self.handle_rtcp_packet(packet).await
    }

    /// Terminate peer connection and release all resources
    pub async fn close(&self) -> Result<()> {
        self.is_closed.store(true, Ordering::SeqCst);
        let mut state_guard = self.state.lock().await;
        *state_guard = SessionState::Closed;

        let mut pending_guard = self.pending_rtp_packets.lock().await;
        pending_guard.clear();

        Ok(())
    }

    /// Synchronously signal session closure and clear queued packets without async await
    pub fn close_sync(&self) {
        self.is_closed.store(true, Ordering::SeqCst);
        if let Ok(mut state_guard) = self.state.try_lock() {
            *state_guard = SessionState::Closed;
        }
        if let Ok(mut pending_guard) = self.pending_rtp_packets.try_lock() {
            pending_guard.clear();
        }
    }
}
