//! Virtual display screen capture pipeline

pub mod dxgi;
pub mod pipewire;

pub use dxgi::{
    composite_cursor, DxgiCapturer, PointerPosition, PointerShape, PointerShapeType,
};
pub use pipewire::{
    composite_pipewire_cursor, PipeWireCapturer, PipeWireCursorMetadata, PipeWireCursorMode,
    PipeWirePixelFormat,
};

#[cfg(target_os = "windows")]
pub type DefaultScreenCapturer = DxgiCapturer;

#[cfg(not(target_os = "windows"))]
pub type DefaultScreenCapturer = PipeWireCapturer;

use anyhow::Result;

/// Captured screen frame representation
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedFrame {
    /// Frame width in pixels
    pub width: u32,
    /// Frame height in pixels
    pub height: u32,
    /// Bytes per row (including padding if present)
    pub stride: u32,
    /// Raw pixel bytes in 32-bit RGBA format
    pub data: Vec<u8>,
    /// Frame capture timestamp in microseconds
    pub timestamp_us: u64,
}

impl CapturedFrame {
    /// Calculate total buffer size expected from stride and height
    pub fn buffer_size(&self) -> usize {
        (self.stride as usize) * (self.height as usize)
    }

    /// Check whether the frame dimensions and buffer are valid
    pub fn is_valid(&self) -> bool {
        self.width > 0
            && self.height > 0
            && self.stride >= self.width * 4
            && self.data.len() >= self.buffer_size()
    }

    /// Get pixel at coordinate (x, y) as RGBA [r, g, b, a]
    pub fn get_pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x >= self.width || y >= self.height || !self.is_valid() {
            return None;
        }
        let offset = ((y as usize) * (self.stride as usize)) + ((x as usize) * 4);
        if offset + 4 > self.data.len() {
            return None;
        }
        Some([
            self.data[offset],
            self.data[offset + 1],
            self.data[offset + 2],
            self.data[offset + 3],
        ])
    }
}

/// Core interface for capturing desktop frames
pub trait ScreenCapturer: Send + Sync {
    /// Start capturing from a specified display adapter output index.
    fn start(&mut self, output_index: u32) -> Result<()>;

    /// Capture the next available frame within the specified timeout in milliseconds.
    ///
    /// Returns:
    /// - `Ok(Some(frame))` if a new frame was successfully captured.
    /// - `Ok(None)` if the timeout expired without a new frame (e.g. desktop unchanged).
    /// - `Err(e)` if an unrecoverable capture error or device loss occurred.
    fn capture_frame(&mut self, timeout_ms: u32) -> Result<Option<CapturedFrame>>;

    /// Stop capturing and release all hardware resources.
    fn stop(&mut self) -> Result<()>;
}
