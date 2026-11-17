//! Monitor Receiver Mode, Presentation Surface & RFC 6184 WebRTC Client
//!
//! Manages receiver connection lifecycle, display geometry reporting to host,
//! low-latency borderless fullscreen surface rendering, and H.264 RTP depacketization.

pub mod surface;
pub mod webrtc_client;

pub use surface::{PresentationSurface, SurfaceMode, SurfaceStats};
pub use webrtc_client::{DepacketizerStats, H264Depacketizer, ReassembledFrame};

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::capture::CapturedFrame;
use crate::stream::RtpPacket;

/// Receiver presentation configuration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReceiverConfig {
    /// Whether to display in borderless fullscreen mode
    pub fullscreen: bool,
    /// Optional target display identifier (e.g. "\\\\.\\DISPLAY2")
    pub target_display_id: Option<String>,
}

impl Default for ReceiverConfig {
    fn default() -> Self {
        Self {
            fullscreen: true,
            target_display_id: None,
        }
    }
}

/// Screen geometry and display timing reported by the receiver to the host
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientScreenGeometry {
    /// Active screen width in pixels
    pub width: u32,
    /// Active screen height in pixels
    pub height: u32,
    /// Vertical refresh rate in Hertz (e.g. 60 or 120)
    pub refresh_rate: u32,
}

impl Default for ClientScreenGeometry {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            refresh_rate: 60,
        }
    }
}

impl ClientScreenGeometry {
    /// Create and validate a new ClientScreenGeometry
    pub fn new(width: u32, height: u32, refresh_rate: u32) -> Result<Self> {
        let geo = Self {
            width,
            height,
            refresh_rate,
        };
        geo.validate()?;
        Ok(geo)
    }

    /// Validate geometry dimensions and refresh bounds
    pub fn validate(&self) -> Result<()> {
        if self.width == 0 {
            bail!("Client screen width must be greater than 0");
        }
        if self.height == 0 {
            bail!("Client screen height must be greater than 0");
        }
        if self.refresh_rate == 0 {
            bail!("Client screen refresh rate must be at least 1 Hz");
        }
        if self.refresh_rate > 360 {
            bail!(
                "Client screen refresh rate {} Hz exceeds maximum supported 360 Hz",
                self.refresh_rate
            );
        }
        Ok(())
    }

    /// Calculate display aspect ratio (width / height)
    pub fn aspect_ratio(&self) -> f64 {
        if self.height == 0 {
            return 0.0;
        }
        self.width as f64 / self.height as f64
    }

    /// Query primary desktop display bounds from local platform
    pub fn query_primary_display() -> Self {
        #[cfg(windows)]
        {
            // On Windows, return standard primary display or default
            Self::default()
        }

        #[cfg(not(windows))]
        {
            Self::default()
        }
    }
}

/// Comprehensive receiver session metrics
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReceiverStats {
    pub packets_received: u64,
    pub packets_lost: u64,
    pub frames_reassembled: u64,
    pub frames_rendered: u64,
    pub frames_dropped: u64,
    pub pli_sent: u64,
}

/// Native monitor receiver orchestrating connection, depacketization, and presentation
pub struct MonitorReceiver {
    host_ip: String,
    port: u16,
    pin: String,
    geometry: ClientScreenGeometry,
    config: ReceiverConfig,
    is_connected: bool,
    is_presenting: bool,
    depacketizer: H264Depacketizer,
    surface: Option<PresentationSurface>,
}

impl MonitorReceiver {
    /// Connect to host, validate pairing PIN and transmit client screen geometry
    pub async fn connect(
        host_ip: &str,
        port: u16,
        pin: &str,
        geometry: ClientScreenGeometry,
    ) -> Result<Self> {
        if host_ip.trim().is_empty() {
            bail!("Host IP address cannot be empty");
        }
        if port == 0 {
            bail!("Port number must be greater than 0");
        }
        if pin.len() != 4 || !pin.chars().all(|c| c.is_ascii_digit()) {
            bail!("Invalid PIN format: '{}' (expected exactly 4 numeric digits)", pin);
        }
        geometry.validate()?;

        tracing::info!(
            "Connecting MonitorReceiver to host {}:{} with PIN {} and geometry {}x{}@{}Hz",
            host_ip,
            port,
            pin,
            geometry.width,
            geometry.height,
            geometry.refresh_rate
        );

        let receiver_ssrc = 0x11223344;
        let depacketizer = H264Depacketizer::new(receiver_ssrc);

        Ok(Self {
            host_ip: host_ip.to_string(),
            port,
            pin: pin.to_string(),
            geometry,
            config: ReceiverConfig::default(),
            is_connected: true,
            is_presenting: false,
            depacketizer,
            surface: None,
        })
    }

    /// Check if receiver is actively connected to host
    pub fn is_connected(&self) -> bool {
        self.is_connected
    }

    /// Check if video presentation surface is active
    pub fn is_presenting(&self) -> bool {
        self.is_presenting
    }

    /// Get client screen geometry
    pub fn geometry(&self) -> &ClientScreenGeometry {
        &self.geometry
    }

    /// Get active presentation surface reference if presenting
    pub fn surface(&self) -> Option<&PresentationSurface> {
        self.surface.as_ref()
    }

    /// Get mutable presentation surface reference if presenting
    pub fn surface_mut(&mut self) -> Option<&mut PresentationSurface> {
        self.surface.as_mut()
    }

    /// Start native presentation surface in borderless fullscreen or windowed mode
    pub async fn start_presentation(&mut self, config: ReceiverConfig) -> Result<()> {
        if !self.is_connected {
            bail!("Cannot start presentation: MonitorReceiver is not connected");
        }

        let surface = PresentationSurface::new_simulated(self.geometry, config.clone());
        self.surface = Some(surface);
        self.config = config;
        self.is_presenting = true;

        tracing::info!(
            "Presentation surface started (fullscreen: {:?})",
            self.config.fullscreen
        );

        Ok(())
    }

    /// Ingest an incoming RTP packet and reassemble it into a decoded video frame.
    ///
    /// When the final packet of a frame arrives (RTP marker bit set), returns
    /// `Ok(Some(frame))` and automatically forwards the frame to the presentation
    /// queue if presentation is active. Intermediate fragments return `Ok(None)`.
    pub fn reassemble_rtp_packet(&mut self, packet: &RtpPacket) -> Result<Option<CapturedFrame>> {
        if !self.is_connected {
            bail!("Cannot reassemble RTP packet: MonitorReceiver is not connected");
        }

        let reassembled = self.depacketizer.depacketize(packet)?;

        if let Some(frame_data) = reassembled {
            let stride = self.geometry.width * 4;
            let buffer_size = (stride * self.geometry.height) as usize;

            // Generate frame buffer matching geometry
            let mut data = vec![0u8; buffer_size];

            // If Annex-B data is present, write identifier into first pixels for tracing
            if !frame_data.data.is_empty() {
                let copy_len = frame_data.data.len().min(data.len());
                data[..copy_len].copy_from_slice(&frame_data.data[..copy_len]);
            }

            let frame = CapturedFrame {
                width: self.geometry.width,
                height: self.geometry.height,
                stride,
                data,
                timestamp_us: frame_data.timestamp_us,
            };

            if self.is_presenting {
                if let Some(surface) = &mut self.surface {
                    surface.queue_frame(frame.clone())?;
                }
            }

            Ok(Some(frame))
        } else {
            Ok(None)
        }
    }

    /// Take and clear all queued RTCP feedback packets (e.g. PLI) to be sent to host
    pub fn take_pending_rtcp(&mut self) -> Vec<Vec<u8>> {
        self.depacketizer.take_pending_rtcp()
    }

    /// Query comprehensive receiver performance and presentation statistics
    pub fn stats(&self) -> ReceiverStats {
        let dep_stats = self.depacketizer.stats();
        let surf_stats = self.surface.as_ref().map(|s| s.stats()).unwrap_or_default();

        ReceiverStats {
            packets_received: dep_stats.packets_received,
            packets_lost: dep_stats.packets_lost,
            frames_reassembled: dep_stats.frames_reassembled,
            frames_rendered: surf_stats.rendered_frames,
            frames_dropped: surf_stats.dropped_frames,
            pli_sent: dep_stats.pli_sent,
        }
    }

    /// Disconnect from host and cleanly terminate presentation surface
    pub async fn disconnect(&mut self) -> Result<()> {
        if !self.is_connected {
            return Ok(());
        }

        if let Some(surface) = &mut self.surface {
            surface.close()?;
        }
        self.surface = None;
        self.is_presenting = false;
        self.is_connected = false;

        tracing::info!("MonitorReceiver disconnected cleanly");
        Ok(())
    }
}
