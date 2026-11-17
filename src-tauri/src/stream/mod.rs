//! Video encoding and WebRTC streaming

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamConfig {
    pub target_bitrate_kbps: u32,
    pub max_framerate: u32,
}
