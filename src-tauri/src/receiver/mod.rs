//! Monitor receiver mode and rendering surface

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiverConfig {
    pub fullscreen: bool,
    pub target_display_id: Option<String>,
}
