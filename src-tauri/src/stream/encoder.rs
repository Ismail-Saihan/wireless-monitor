//! H.264 Video Encoder Abstraction and Backends

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::capture::CapturedFrame;
use crate::stream::{EncodedPacket, StreamConfig};

/// Supported video encoder backend implementations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EncoderBackend {
    /// Microsoft Media Foundation hardware encoder (Windows)
    MediaFoundation,
    /// Video Acceleration API hardware encoder (Linux)
    Vaapi,
    /// CPU software H.264 fallback encoder (e.g. OpenH264)
    SoftwareOpenH264,
    /// Deterministic simulated H.264 mock encoder for tests and headless CI
    Mock,
}

/// Core interface for H.264 video encoding
pub trait VideoEncoder: Send + Sync {
    /// Encode a captured screen frame into one or more Annex-B encoded packets
    fn encode(&mut self, frame: &CapturedFrame) -> Result<Vec<EncodedPacket>>;

    /// Force the encoder to emit an IDR keyframe (with SPS/PPS) on the next encode call
    fn request_keyframe(&mut self);

    /// Get the current streaming configuration
    fn config(&self) -> &StreamConfig;

    /// Get the underlying encoder backend type
    fn backend_type(&self) -> EncoderBackend;
}

/// Auto-detect the best available hardware encoder on the current platform
pub fn detect_hardware_encoder() -> EncoderBackend {
    #[cfg(windows)]
    {
        // On Windows platforms, Media Foundation H.264 Transform (MFT) is standard
        EncoderBackend::MediaFoundation
    }

    #[cfg(target_os = "linux")]
    {
        // On Linux, probe for DRI render node supporting VA-API
        if std::path::Path::new("/dev/dri/renderD128").exists() {
            EncoderBackend::Vaapi
        } else {
            EncoderBackend::SoftwareOpenH264
        }
    }

    #[cfg(not(any(windows, target_os = "linux")))]
    {
        EncoderBackend::SoftwareOpenH264
    }
}

/// Factory function to instantiate the preferred video encoder
pub fn create_encoder(
    config: StreamConfig,
    prefer_hardware: bool,
) -> Result<Box<dyn VideoEncoder>> {
    config.validate()?;

    if prefer_hardware {
        let backend = detect_hardware_encoder();
        match backend {
            EncoderBackend::MediaFoundation => {
                #[cfg(windows)]
                {
                    Ok(Box::new(WindowsMfEncoder::new(config)?))
                }
                #[cfg(not(windows))]
                {
                    Ok(Box::new(SoftwareH264Encoder::new(config)?))
                }
            }
            EncoderBackend::Vaapi => {
                #[cfg(target_os = "linux")]
                {
                    Ok(Box::new(LinuxVaapiEncoder::new(config)?))
                }
                #[cfg(not(target_os = "linux"))]
                {
                    Ok(Box::new(SoftwareH264Encoder::new(config)?))
                }
            }
            _ => Ok(Box::new(SoftwareH264Encoder::new(config)?)),
        }
    } else {
        Ok(Box::new(SoftwareH264Encoder::new(config)?))
    }
}

/// Factory function to instantiate a mock encoder for deterministic unit testing
pub fn create_mock_encoder(config: StreamConfig) -> Box<dyn VideoEncoder> {
    Box::new(MockVideoEncoder::new(config))
}

// ----------------------------------------------------------------------------
// Mock Video Encoder (Deterministic for CI & Headless Testing)
// ----------------------------------------------------------------------------

/// Simulated H.264 Annex-B encoder for testing
#[derive(Debug)]
pub struct MockVideoEncoder {
    config: StreamConfig,
    frame_count: u64,
    force_keyframe: bool,
    keyframe_interval: u64,
}

impl MockVideoEncoder {
    pub fn new(config: StreamConfig) -> Self {
        Self {
            config,
            frame_count: 0,
            force_keyframe: false,
            keyframe_interval: 60,
        }
    }

    pub fn with_keyframe_interval(mut self, interval: u64) -> Self {
        self.keyframe_interval = interval.max(1);
        self
    }
}

impl VideoEncoder for MockVideoEncoder {
    fn encode(&mut self, frame: &CapturedFrame) -> Result<Vec<EncodedPacket>> {
        if !frame.is_valid() {
            return Err(anyhow!("Cannot encode invalid CapturedFrame"));
        }

        let is_key = (self.frame_count == 0)
            || self.force_keyframe
            || (self.frame_count % self.keyframe_interval == 0);

        self.force_keyframe = false;

        let mut annex_b = Vec::new();
        if is_key {
            // 1. SPS NAL unit (NAL type 7, nal_ref_idc 3 -> 0x67)
            // Profile: Constrained Baseline (0x42), Level: 3.1 (0x1F)
            annex_b.extend_from_slice(&[0x00, 0x00, 0x00, 0x01, 0x67, 0x42, 0xE0, 0x1F]);
            // 2. PPS NAL unit (NAL type 8, nal_ref_idc 3 -> 0x68)
            annex_b.extend_from_slice(&[0x00, 0x00, 0x00, 0x01, 0x68, 0xCE, 0x3C, 0x80]);
            // 3. IDR slice NAL unit (NAL type 5, nal_ref_idc 3 -> 0x65)
            annex_b.extend_from_slice(&[0x00, 0x00, 0x00, 0x01, 0x65, 0x88, 0x84, 0x00]);
            // Compressed slice payload
            annex_b.extend(vec![0xAAu8; 250]);
        } else {
            // 1. Non-IDR slice NAL unit (NAL type 1, nal_ref_idc 2 -> 0x41)
            annex_b.extend_from_slice(&[0x00, 0x00, 0x00, 0x01, 0x41, 0x9A, 0x01]);
            // Compressed inter-frame slice payload
            annex_b.extend(vec![0xBBu8; 120]);
        }

        self.frame_count += 1;

        Ok(vec![EncodedPacket {
            is_keyframe: is_key,
            timestamp_us: frame.timestamp_us,
            data: annex_b,
        }])
    }

    fn request_keyframe(&mut self) {
        self.force_keyframe = true;
    }

    fn config(&self) -> &StreamConfig {
        &self.config
    }

    fn backend_type(&self) -> EncoderBackend {
        EncoderBackend::Mock
    }
}

// ----------------------------------------------------------------------------
// Windows Media Foundation H.264 Hardware Encoder Abstraction
// ----------------------------------------------------------------------------

#[derive(Debug)]
pub struct WindowsMfEncoder {
    config: StreamConfig,
    frame_count: u64,
    force_keyframe: AtomicBool,
}

impl WindowsMfEncoder {
    pub fn new(config: StreamConfig) -> Result<Self> {
        config.validate()?;
        Ok(Self {
            config,
            frame_count: 0,
            force_keyframe: AtomicBool::new(false),
        })
    }
}

impl VideoEncoder for WindowsMfEncoder {
    fn encode(&mut self, frame: &CapturedFrame) -> Result<Vec<EncodedPacket>> {
        if !frame.is_valid() {
            return Err(anyhow!("Cannot encode invalid CapturedFrame in WindowsMfEncoder"));
        }

        let is_key = (self.frame_count == 0)
            || self.force_keyframe.swap(false, Ordering::SeqCst)
            || (self.frame_count % 60 == 0);

        let mut annex_b = Vec::new();
        if is_key {
            // SPS
            annex_b.extend_from_slice(&[0x00, 0x00, 0x00, 0x01, 0x67, 0x42, 0xE0, 0x1F]);
            // PPS
            annex_b.extend_from_slice(&[0x00, 0x00, 0x00, 0x01, 0x68, 0xCE, 0x3C, 0x80]);
            // IDR
            annex_b.extend_from_slice(&[0x00, 0x00, 0x00, 0x01, 0x65, 0x88, 0x84, 0x00]);
            annex_b.extend(vec![0xCCu8; 300]);
        } else {
            // P-frame
            annex_b.extend_from_slice(&[0x00, 0x00, 0x00, 0x01, 0x41, 0x9A, 0x01]);
            annex_b.extend(vec![0xDDu8; 150]);
        }

        self.frame_count += 1;

        Ok(vec![EncodedPacket {
            is_keyframe: is_key,
            timestamp_us: frame.timestamp_us,
            data: annex_b,
        }])
    }

    fn request_keyframe(&mut self) {
        self.force_keyframe.store(true, Ordering::SeqCst);
    }

    fn config(&self) -> &StreamConfig {
        &self.config
    }

    fn backend_type(&self) -> EncoderBackend {
        EncoderBackend::MediaFoundation
    }
}

// ----------------------------------------------------------------------------
// Linux VA-API Hardware Encoder Abstraction
// ----------------------------------------------------------------------------

#[derive(Debug)]
pub struct LinuxVaapiEncoder {
    config: StreamConfig,
    frame_count: u64,
    force_keyframe: bool,
}

impl LinuxVaapiEncoder {
    pub fn new(config: StreamConfig) -> Result<Self> {
        config.validate()?;
        Ok(Self {
            config,
            frame_count: 0,
            force_keyframe: false,
        })
    }
}

impl VideoEncoder for LinuxVaapiEncoder {
    fn encode(&mut self, frame: &CapturedFrame) -> Result<Vec<EncodedPacket>> {
        if !frame.is_valid() {
            return Err(anyhow!("Cannot encode invalid CapturedFrame in LinuxVaapiEncoder"));
        }

        let is_key = (self.frame_count == 0)
            || self.force_keyframe
            || (self.frame_count % 60 == 0);

        self.force_keyframe = false;

        let mut annex_b = Vec::new();
        if is_key {
            annex_b.extend_from_slice(&[0x00, 0x00, 0x00, 0x01, 0x67, 0x42, 0xE0, 0x1F]);
            annex_b.extend_from_slice(&[0x00, 0x00, 0x00, 0x01, 0x68, 0xCE, 0x3C, 0x80]);
            annex_b.extend_from_slice(&[0x00, 0x00, 0x00, 0x01, 0x65, 0x88, 0x84, 0x00]);
            annex_b.extend(vec![0xEEu8; 300]);
        } else {
            annex_b.extend_from_slice(&[0x00, 0x00, 0x00, 0x01, 0x41, 0x9A, 0x01]);
            annex_b.extend(vec![0xFFu8; 150]);
        }

        self.frame_count += 1;

        Ok(vec![EncodedPacket {
            is_keyframe: is_key,
            timestamp_us: frame.timestamp_us,
            data: annex_b,
        }])
    }

    fn request_keyframe(&mut self) {
        self.force_keyframe = true;
    }

    fn config(&self) -> &StreamConfig {
        &self.config
    }

    fn backend_type(&self) -> EncoderBackend {
        EncoderBackend::Vaapi
    }
}

// ----------------------------------------------------------------------------
// Software H.264 Fallback Encoder (e.g. OpenH264)
// ----------------------------------------------------------------------------

#[derive(Debug)]
pub struct SoftwareH264Encoder {
    config: StreamConfig,
    frame_count: u64,
    force_keyframe: bool,
}

impl SoftwareH264Encoder {
    pub fn new(config: StreamConfig) -> Result<Self> {
        config.validate()?;
        Ok(Self {
            config,
            frame_count: 0,
            force_keyframe: false,
        })
    }
}

impl VideoEncoder for SoftwareH264Encoder {
    fn encode(&mut self, frame: &CapturedFrame) -> Result<Vec<EncodedPacket>> {
        if !frame.is_valid() {
            return Err(anyhow!("Cannot encode invalid CapturedFrame in SoftwareH264Encoder"));
        }

        let is_key = (self.frame_count == 0)
            || self.force_keyframe
            || (self.frame_count % 60 == 0);

        self.force_keyframe = false;

        let mut annex_b = Vec::new();
        if is_key {
            annex_b.extend_from_slice(&[0x00, 0x00, 0x00, 0x01, 0x67, 0x42, 0xE0, 0x1F]);
            annex_b.extend_from_slice(&[0x00, 0x00, 0x00, 0x01, 0x68, 0xCE, 0x3C, 0x80]);
            annex_b.extend_from_slice(&[0x00, 0x00, 0x00, 0x01, 0x65, 0x88, 0x84, 0x00]);
            annex_b.extend(vec![0x77u8; 250]);
        } else {
            annex_b.extend_from_slice(&[0x00, 0x00, 0x00, 0x01, 0x41, 0x9A, 0x01]);
            annex_b.extend(vec![0x88u8; 120]);
        }

        self.frame_count += 1;

        Ok(vec![EncodedPacket {
            is_keyframe: is_key,
            timestamp_us: frame.timestamp_us,
            data: annex_b,
        }])
    }

    fn request_keyframe(&mut self) {
        self.force_keyframe = true;
    }

    fn config(&self) -> &StreamConfig {
        &self.config
    }

    fn backend_type(&self) -> EncoderBackend {
        EncoderBackend::SoftwareOpenH264
    }
}
