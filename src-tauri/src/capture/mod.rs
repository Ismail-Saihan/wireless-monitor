//! Virtual display screen capture pipeline

use anyhow::Result;

pub struct CapturedFrame {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub data: Vec<u8>,
    pub timestamp_us: u64,
}

pub trait ScreenCapturer: Send + Sync {
    fn start(&mut self, output_index: u32) -> Result<()>;
    fn capture_frame(&mut self, timeout_ms: u32) -> Result<Option<CapturedFrame>>;
    fn stop(&mut self) -> Result<()>;
}
