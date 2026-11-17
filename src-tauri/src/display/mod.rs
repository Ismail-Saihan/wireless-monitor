//! Virtual display creation and lifecycle management

pub mod windows_iddcx;

pub use windows_iddcx::WindowsIddCxManager;

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

/// Display configuration describing virtual monitor resolution and refresh rate
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DisplayConfig {
    pub width: u32,
    pub height: u32,
    pub refresh_rate: u32,
}

impl DisplayConfig {
    /// Create a new display configuration
    pub fn new(width: u32, height: u32, refresh_rate: u32) -> Self {
        Self {
            width,
            height,
            refresh_rate,
        }
    }

    /// Validate the display configuration parameters
    pub fn validate(&self) -> Result<()> {
        if self.width == 0 || self.height == 0 || self.refresh_rate == 0 {
            bail!("Display dimensions and refresh rate must be non-zero");
        }
        if self.width < 320 || self.width > 7680 {
            bail!(
                "Display width {} is outside supported range (320-7680)",
                self.width
            );
        }
        if self.height < 240 || self.height > 4320 {
            bail!(
                "Display height {} is outside supported range (240-4320)",
                self.height
            );
        }
        if self.refresh_rate < 10 || self.refresh_rate > 360 {
            bail!(
                "Refresh rate {} Hz is outside supported range (10-360 Hz)",
                self.refresh_rate
            );
        }
        Ok(())
    }
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            refresh_rate: 60,
        }
    }
}

/// Abstract interface for virtual display adapter lifecycle management
pub trait VirtualDisplayManager: Send + Sync {
    /// Check whether the platform virtual display driver is installed and ready
    fn is_driver_installed(&self) -> Result<bool>;

    /// Run installation routine / driver registration for virtual display driver
    fn install_driver(&self) -> Result<()>;

    /// Create and attach a virtual display with the given configuration.
    /// Returns the allocated monitor ID on success.
    fn create_display(&mut self, config: &DisplayConfig) -> Result<u32>;

    /// Detach and destroy a specific virtual display by monitor ID
    fn destroy_display(&mut self, monitor_id: u32) -> Result<()>;

    /// Detach and destroy all active virtual displays managed by this instance
    fn destroy_all(&mut self) -> Result<()>;
}
