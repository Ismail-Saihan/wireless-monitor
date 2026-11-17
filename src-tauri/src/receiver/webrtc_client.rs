//! WebRTC Client RTP Receiver & RFC 6184 H.264 Depacketization Engine
//!
//! Reassembles Single NAL unit packets and Fragmentation Unit A (FU-A) packets into
//! continuous Annex-B formatted H.264 video frame buffers.
//! Detects RTP sequence gaps and missing fragments, emitting RFC 4585 / RFC 5104
//! RTCP Picture Loss Indication (PLI) and Full Intra Request (FIR) feedback packets.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::stream::RtpPacket;

/// Depacketizer performance and network metrics
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DepacketizerStats {
    /// Total incoming RTP packets received
    pub packets_received: u64,
    /// Total detected dropped or missing packets across sequence gaps
    pub packets_lost: u64,
    /// Total complete Annex-B frames successfully reassembled
    pub frames_reassembled: u64,
    /// Total FU-A fragmentation units processed
    pub fua_fragments_processed: u64,
    /// Total RTCP PLI feedback packets queued for transmission
    pub pli_sent: u64,
}

/// Reassembled video frame containing Annex-B NAL units
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReassembledFrame {
    /// Presentation timestamp in microseconds
    pub timestamp_us: u64,
    /// Whether this frame is an IDR keyframe
    pub is_keyframe: bool,
    /// Annex-B formatted NAL units prefixed by 4-byte start codes
    pub data: Vec<u8>,
}

impl ReassembledFrame {
    /// Extract individual raw NAL units without start codes
    pub fn extract_nalus(&self) -> Vec<&[u8]> {
        let mut nalus = Vec::new();
        let data = &self.data;
        let n = data.len();
        if n < 4 {
            return nalus;
        }

        let mut starts = Vec::new();
        let mut i = 0;
        while i < n - 2 {
            if data[i] == 0 && data[i + 1] == 0 {
                if data[i + 2] == 1 {
                    starts.push((i, 3));
                    i += 3;
                    continue;
                } else if i + 3 < n && data[i + 2] == 0 && data[i + 3] == 1 {
                    starts.push((i, 4));
                    i += 4;
                    continue;
                }
            }
            i += 1;
        }

        for (idx, &(pos, sc_len)) in starts.iter().enumerate() {
            let nalu_start = pos + sc_len;
            let nalu_end = if idx + 1 < starts.len() {
                starts[idx + 1].0
            } else {
                n
            };
            if nalu_start <= nalu_end {
                nalus.push(&data[nalu_start..nalu_end]);
            }
        }

        nalus
    }

    /// Check if reassembled frame contains SPS and PPS
    pub fn contains_sps_pps(&self) -> bool {
        let nalus = self.extract_nalus();
        let has_sps = nalus
            .iter()
            .any(|nalu| !nalu.is_empty() && (nalu[0] & 0x1F) == 7);
        let has_pps = nalus
            .iter()
            .any(|nalu| !nalu.is_empty() && (nalu[0] & 0x1F) == 8);
        has_sps && has_pps
    }
}

/// RFC 6184 H.264 RTP Depacketizer with sequence gap loss recovery
#[derive(Debug)]
pub struct H264Depacketizer {
    /// Local receiver SSRC for RTCP feedback
    ssrc: u32,
    /// Expected next RTP sequence number (16-bit wrapping)
    expected_seq: Option<u16>,
    /// Accumulated Annex-B buffer for the current frame
    current_frame_annex_b: Vec<u8>,
    /// In-progress FU-A fragment accumulator
    current_fua: Vec<u8>,
    /// In-progress FU-A NAL unit type
    current_fua_nalu_type: Option<u8>,
    /// Flag indicating if any packet was lost in current frame
    has_gap_in_frame: bool,
    /// Pending outgoing RTCP feedback packets (e.g. PLI)
    pending_rtcp: Vec<Vec<u8>>,
    /// Depacketizer metrics
    stats: DepacketizerStats,
    /// RTP clock rate (standard 90 kHz for video)
    clock_rate: u32,
}

impl H264Depacketizer {
    /// Create a new depacketizer with the specified receiver SSRC
    pub fn new(ssrc: u32) -> Self {
        Self {
            ssrc,
            expected_seq: None,
            current_frame_annex_b: Vec::with_capacity(65536),
            current_fua: Vec::with_capacity(32768),
            current_fua_nalu_type: None,
            has_gap_in_frame: false,
            pending_rtcp: Vec::new(),
            stats: DepacketizerStats::default(),
            clock_rate: 90_000,
        }
    }

    /// Get current metrics snapshot
    pub fn stats(&self) -> DepacketizerStats {
        self.stats.clone()
    }

    /// Take and clear all pending RTCP feedback packets
    pub fn take_pending_rtcp(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.pending_rtcp)
    }

    /// Generate an RFC 4585 Picture Loss Indication (PLI) feedback packet
    pub fn generate_pli(&self, media_ssrc: u32) -> Vec<u8> {
        let mut packet = Vec::with_capacity(12);
        // Byte 0: V=2, P=0, FMT=1 (PLI) -> 0x81
        packet.push(0x81);
        // Byte 1: PT=206 (Payload-Specific Feedback)
        packet.push(206);
        // Bytes 2-3: Length = 2 words (12 bytes total)
        packet.extend_from_slice(&2u16.to_be_bytes());
        // Bytes 4-7: SSRC of sender (receiver)
        packet.extend_from_slice(&self.ssrc.to_be_bytes());
        // Bytes 8-11: SSRC of media source (host)
        packet.extend_from_slice(&media_ssrc.to_be_bytes());
        packet
    }

    /// Generate an RFC 5104 Full Intra Request (FIR) feedback packet
    pub fn generate_fir(&self, media_ssrc: u32, seq_nr: u8) -> Vec<u8> {
        let mut packet = Vec::with_capacity(20);
        // Byte 0: V=2, P=0, FMT=4 (FIR) -> 0x84
        packet.push(0x84);
        // Byte 1: PT=206 (PSFB)
        packet.push(206);
        // Bytes 2-3: Length = 4 words (20 bytes total)
        packet.extend_from_slice(&4u16.to_be_bytes());
        // Bytes 4-7: SSRC of sender
        packet.extend_from_slice(&self.ssrc.to_be_bytes());
        // Bytes 8-11: SSRC of media source
        packet.extend_from_slice(&media_ssrc.to_be_bytes());
        // Bytes 12-15: SSRC of media sender
        packet.extend_from_slice(&media_ssrc.to_be_bytes());
        // Byte 16: Sequence number of FIR
        packet.push(seq_nr);
        // Bytes 17-19: Reserved (0)
        packet.extend_from_slice(&[0u8; 3]);
        packet
    }

    /// Ingest an RTP packet, reassemble Annex-B NALUs, and detect sequence gaps
    pub fn depacketize(&mut self, packet: &RtpPacket) -> Result<Option<ReassembledFrame>> {
        self.stats.packets_received += 1;
        let seq = packet.sequence_number;

        let mut gap_detected_this_packet = false;

        // Sequence number gap detection (16-bit sequence tracking)
        if let Some(expected) = self.expected_seq {
            if seq != expected {
                let lost = seq.wrapping_sub(expected);
                self.stats.packets_lost += lost as u64;
                self.has_gap_in_frame = true;
                gap_detected_this_packet = true;
                self.stats.pli_sent += 1;
                self.pending_rtcp.push(self.generate_pli(packet.ssrc));

                // Clear any in-progress FU-A assembly due to sequence loss
                self.current_fua.clear();
                self.current_fua_nalu_type = None;
            }
        }

        self.expected_seq = Some(seq.wrapping_add(1));

        if packet.payload.is_empty() {
            return Ok(None);
        }

        let nalu_indicator = packet.payload[0];
        let nalu_type = nalu_indicator & 0x1F;

        if (1..=23).contains(&nalu_type) {
            // Single NAL unit packet (RFC 6184 Section 5.6)
            self.current_frame_annex_b
                .extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);
            self.current_frame_annex_b
                .extend_from_slice(&packet.payload);
        } else if nalu_type == 28 {
            // Fragmentation Unit A (FU-A) packet (RFC 6184 Section 5.8)
            if packet.payload.len() < 2 {
                bail!("Malformed FU-A packet: payload shorter than 2 bytes");
            }

            self.stats.fua_fragments_processed += 1;

            let fu_header = packet.payload[1];
            let is_start = (fu_header & 0x80) != 0;
            let is_end = (fu_header & 0x40) != 0;
            let fu_nalu_type = fu_header & 0x1F;
            let ref_idc = nalu_indicator & 0x60;
            let reconstructed_header = ref_idc | fu_nalu_type;

            if is_start {
                // Initialize fragmented NAL unit with 4-byte Annex-B start code
                self.current_fua.clear();
                self.current_fua
                    .extend_from_slice(&[0x00, 0x00, 0x00, 0x01, reconstructed_header]);
                self.current_fua.extend_from_slice(&packet.payload[2..]);
                self.current_fua_nalu_type = Some(fu_nalu_type);
            } else {
                if self.current_fua.is_empty() {
                    // Fragment arrived without start packet
                    if !gap_detected_this_packet {
                        self.stats.packets_lost += 1;
                        self.stats.pli_sent += 1;
                        self.pending_rtcp.push(self.generate_pli(packet.ssrc));
                    }
                    self.has_gap_in_frame = true;
                    return Ok(None);
                }
                self.current_fua.extend_from_slice(&packet.payload[2..]);
            }

            if is_end && !self.current_fua.is_empty() {
                self.current_frame_annex_b
                    .extend_from_slice(&self.current_fua);
                self.current_fua.clear();
                self.current_fua_nalu_type = None;
            }
        }

        // Check RTP Marker bit indicating access unit (frame) boundary
        if packet.marker {
            if self.has_gap_in_frame || self.current_frame_annex_b.is_empty() {
                // Drop incomplete / corrupted frame and await IDR keyframe
                self.current_frame_annex_b.clear();
                self.has_gap_in_frame = false;
                return Ok(None);
            }

            let timestamp_us =
                ((packet.timestamp as u128 * 1_000_000) / (self.clock_rate as u128)) as u64;

            // Scan for IDR (type 5) or SPS (type 7) to determine keyframe flag
            let is_keyframe = self.is_keyframe_data(&self.current_frame_annex_b);

            let frame_data = std::mem::take(&mut self.current_frame_annex_b);
            self.has_gap_in_frame = false;
            self.stats.frames_reassembled += 1;

            Ok(Some(ReassembledFrame {
                timestamp_us,
                is_keyframe,
                data: frame_data,
            }))
        } else {
            Ok(None)
        }
    }

    /// Check if raw Annex-B bytes contain an IDR (type 5) or SPS (type 7) slice
    fn is_keyframe_data(&self, data: &[u8]) -> bool {
        let n = data.len();
        if n < 5 {
            return false;
        }

        let mut i = 0;
        while i < n - 4 {
            if data[i] == 0 && data[i + 1] == 0 {
                let sc_len = if data[i + 2] == 1 {
                    3
                } else if i + 3 < n && data[i + 2] == 0 && data[i + 3] == 1 {
                    4
                } else {
                    i += 1;
                    continue;
                };

                let nalu_type = data[i + sc_len] & 0x1F;
                if nalu_type == 5 || nalu_type == 7 {
                    return true;
                }
                i += sc_len;
                continue;
            }
            i += 1;
        }

        false
    }
}
