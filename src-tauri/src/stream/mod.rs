//! Video encoding and WebRTC streaming subsystem

pub mod encoder;
pub mod webrtc_session;

pub use encoder::{
    create_encoder, create_mock_encoder, detect_hardware_encoder, EncoderBackend,
    MockVideoEncoder, VideoEncoder,
};
pub use webrtc_session::{
    RtpPacket, RtpPacketizer, SessionState, SessionStats, WebRtcHostSession,
};

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

/// Video stream encoding and transmission configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamConfig {
    /// Target video encoding bitrate in kilobits per second (kbps)
    pub target_bitrate_kbps: u32,
    /// Maximum framerate in frames per second (fps)
    pub max_framerate: u32,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            target_bitrate_kbps: 10_000,
            max_framerate: 60,
        }
    }
}

impl StreamConfig {
    /// Validate configuration bounds
    pub fn validate(&self) -> Result<()> {
        if self.target_bitrate_kbps == 0 {
            bail!("target_bitrate_kbps must be greater than 0");
        }
        if self.target_bitrate_kbps > 200_000 {
            bail!("target_bitrate_kbps cannot exceed 200,000 kbps (200 Mbps)");
        }
        if self.max_framerate == 0 {
            bail!("max_framerate must be at least 1 fps");
        }
        if self.max_framerate > 240 {
            bail!("max_framerate cannot exceed 240 fps");
        }
        Ok(())
    }
}

/// Encoded video packet containing Annex-B formatted NAL units
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncodedPacket {
    /// Whether this packet represents an instantaneous decoder refresh (IDR) keyframe
    pub is_keyframe: bool,
    /// Presentation timestamp in microseconds
    pub timestamp_us: u64,
    /// Raw Annex-B NAL units prefixed with 3-byte or 4-byte start codes
    pub data: Vec<u8>,
}

impl EncodedPacket {
    /// Extract individual raw NAL units without their start codes
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

    /// Count total NAL units present in the packet
    pub fn nalu_count(&self) -> usize {
        self.extract_nalus().len()
    }

    /// Check if the packet contains Sequence Parameter Set (SPS) and Picture Parameter Set (PPS)
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
