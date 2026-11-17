//! Virtual display creation and lifecycle management

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayConfig {
    pub width: u32,
    pub height: u32,
    pub refresh_rate: u32,
}

pub trait VirtualDisplayManager: Send + Sync {
    fn is_driver_installed(&self) -> Result<bool>;
    fn install_driver(&self) -> Result<()>;
    fn create_display(&mut self, config: &DisplayConfig) -> Result<u32>;
    fn destroy_display(&mut self, monitor_id: u32) -> Result<()>;
    fn destroy_all(&mut self) -> Result<()>;
}
