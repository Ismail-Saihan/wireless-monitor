//! Configurable stream settings and dynamic resolution handshake management

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::display::DisplayConfig;
use crate::receiver::ClientScreenGeometry;
use crate::stream::StreamConfig;

/// Configurable video streaming settings and client resolution negotiation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StreamSettings {
    /// Resolution mode: "auto", "1080p", "1440p", "4k", or "custom"
    pub resolution_mode: String,
    /// Explicit pixel width for "custom" resolution mode
    pub custom_width: Option<u32>,
    /// Explicit pixel height for "custom" resolution mode
    pub custom_height: Option<u32>,
    /// Target stream framerate in frames per second (typically 30 or 60)
    pub framerate: u32,
    /// Target video encoding bitrate in kilobits per second (kbps)
    pub bitrate_kbps: u32,
}

impl Default for StreamSettings {
    fn default() -> Self {
        Self {
            resolution_mode: "auto".to_string(),
            custom_width: None,
            custom_height: None,
            framerate: 60,
            bitrate_kbps: 20_000,
        }
    }
}

impl StreamSettings {
    /// Create a new StreamSettings configuration
    pub fn new(
        resolution_mode: &str,
        custom_width: Option<u32>,
        custom_height: Option<u32>,
        framerate: u32,
        bitrate_kbps: u32,
    ) -> Self {
        Self {
            resolution_mode: resolution_mode.to_string(),
            custom_width,
            custom_height,
            framerate,
            bitrate_kbps,
        }
    }

    /// Validate configuration bounds, custom dimension parity, and bitrate limits
    pub fn validate(&self) -> Result<()> {
        let mode = self.resolution_mode.trim().to_lowercase();
        match mode.as_str() {
            "auto" | "1080p" | "1440p" | "4k" => {}
            "custom" => {
                let w = self
                    .custom_width
                    .ok_or_else(|| anyhow::anyhow!("custom_width is required when resolution_mode is 'custom'"))?;
                let h = self
                    .custom_height
                    .ok_or_else(|| anyhow::anyhow!("custom_height is required when resolution_mode is 'custom'"))?;

                if w % 2 != 0 || h % 2 != 0 {
                    bail!("Custom width and height must be even numbers for video streaming");
                }
                if !(640..=7680).contains(&w) {
                    bail!("Custom width {} outside supported range (640-7680)", w);
                }
                if !(480..=4320).contains(&h) {
                    bail!("Custom height {} outside supported range (480-4320)", h);
                }
            }
            _ => {
                bail!(
                    "Invalid resolution mode '{}'. Expected 'auto', '1080p', '1440p', '4k', or 'custom'.",
                    self.resolution_mode
                );
            }
        }

        if !(1..=360).contains(&self.framerate) {
            bail!(
                "Framerate {} fps outside supported range (1-360 fps)",
                self.framerate
            );
        }

        if !(1000..=100_000).contains(&self.bitrate_kbps) {
            bail!(
                "Bitrate {} kbps outside supported range (1000-100000 kbps)",
                self.bitrate_kbps
            );
        }

        Ok(())
    }

    /// Dynamically resolve virtual display configuration based on mode and client geometry
    pub fn resolve_display_config(&self, client_geom: &ClientScreenGeometry) -> DisplayConfig {
        let mode = self.resolution_mode.trim().to_lowercase();
        let target_fps = if self.framerate > 0 {
            self.framerate
        } else if client_geom.refresh_rate > 0 {
            client_geom.refresh_rate
        } else {
            60
        };

        let (width, height) = match mode.as_str() {
            "1080p" => (1920, 1080),
            "1440p" => (2560, 1440),
            "4k" => (3840, 2160),
            "custom" => {
                let w = self.custom_width.unwrap_or(1920);
                let h = self.custom_height.unwrap_or(1080);
                ((w / 2) * 2, (h / 2) * 2)
            }
            "auto" | _ => {
                let base_w = if client_geom.width > 0 {
                    client_geom.width
                } else {
                    1920
                };
                let base_h = if client_geom.height > 0 {
                    client_geom.height
                } else {
                    1080
                };
                // Ensure even dimensions for H.264 macroblock constraints
                ((base_w / 2) * 2, (base_h / 2) * 2)
            }
        };

        DisplayConfig::new(width, height, target_fps)
    }

    /// Convert StreamSettings to underlying video encoder StreamConfig
    pub fn to_stream_config(&self) -> StreamConfig {
        StreamConfig {
            target_bitrate_kbps: self.bitrate_kbps,
            max_framerate: self.framerate,
        }
    }
}
