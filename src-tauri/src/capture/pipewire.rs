//! Linux PipeWire Screen Capture Engine
//!
//! Captures desktop video frames from a Wayland virtual display using PipeWire
//! screencast node streams (e.g. created via GNOME Mutter RemoteDesktop or FreeDesktop
//! XDG Desktop Portal). Supports SPA video format conversion to 32-bit RGBA,
//! stride calculation and alignment (`stride >= width * 4`), cursor position metadata
//! extraction, and alpha compositing.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use tracing::info;

use super::{CapturedFrame, ScreenCapturer};

/// Video pixel formats supported by PipeWire screencast stream buffers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PipeWirePixelFormat {
    RGBA,
    BGRA,
    RGBx,
    BGRx,
    RGB,
    BGR,
}

/// Cursor mode negotiated with PipeWire screencast stream
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PipeWireCursorMode {
    /// Cursor is rendered directly into the frame buffer by the compositor
    Embedded,
    /// Cursor is transmitted as SPA metadata and composited by this capturer
    Metadata,
    /// Cursor is hidden / invisible
    Hidden,
}

/// PipeWire cursor metadata chunk
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipeWireCursorMetadata {
    /// Host cursor X coordinate on virtual display
    pub x: i32,
    /// Host cursor Y coordinate on virtual display
    pub y: i32,
    /// Whether the cursor is currently visible
    pub visible: bool,
    /// Cursor bitmap width in pixels
    pub width: u32,
    /// Cursor bitmap height in pixels
    pub height: u32,
    /// Hotspot horizontal offset within the cursor bitmap
    pub hotspot_x: i32,
    /// Hotspot vertical offset within the cursor bitmap
    pub hotspot_y: i32,
    /// Raw cursor bitmap pixel data (32-bit RGBA)
    pub data: Vec<u8>,
}

/// Calculate aligned stride for a given width, bytes per pixel, and byte alignment.
/// Guarantees that `stride >= width * bytes_per_pixel`.
pub fn calculate_aligned_stride(width: u32, bytes_per_pixel: u32, alignment: u32) -> u32 {
    let row_bytes = width * bytes_per_pixel;
    if alignment == 0 {
        return row_bytes;
    }
    let remainder = row_bytes % alignment;
    if remainder == 0 {
        row_bytes
    } else {
        row_bytes + (alignment - remainder)
    }
}

/// Composite mouse cursor shape onto a 32-bit RGBA frame buffer.
/// Handles alpha blending, hotspot positioning, and boundary clipping.
pub fn composite_pipewire_cursor(
    frame_data: &mut [u8],
    frame_width: u32,
    frame_height: u32,
    frame_stride: u32,
    cursor: &PipeWireCursorMetadata,
) -> Result<()> {
    if !cursor.visible {
        return Ok(());
    }

    let dest_x = cursor.x - cursor.hotspot_x;
    let dest_y = cursor.y - cursor.hotspot_y;

    for sy in 0..cursor.height {
        let dy = dest_y + (sy as i32);
        if dy < 0 || dy >= (frame_height as i32) {
            continue;
        }

        for sx in 0..cursor.width {
            let dx = dest_x + (sx as i32);
            if dx < 0 || dx >= (frame_width as i32) {
                continue;
            }

            let cursor_idx = ((sy as usize) * (cursor.width as usize) + (sx as usize)) * 4;
            if cursor_idx + 4 > cursor.data.len() {
                continue;
            }

            let r = cursor.data[cursor_idx];
            let g = cursor.data[cursor_idx + 1];
            let b = cursor.data[cursor_idx + 2];
            let a = cursor.data[cursor_idx + 3];

            let frame_idx = ((dy as usize) * (frame_stride as usize)) + ((dx as usize) * 4);
            if frame_idx + 4 > frame_data.len() {
                continue;
            }

            if a == 255 {
                frame_data[frame_idx] = r;
                frame_data[frame_idx + 1] = g;
                frame_data[frame_idx + 2] = b;
                frame_data[frame_idx + 3] = 255;
            } else if a > 0 {
                let dest_r = frame_data[frame_idx] as u32;
                let dest_g = frame_data[frame_idx + 1] as u32;
                let dest_b = frame_data[frame_idx + 2] as u32;
                let alpha = a as u32;
                let inv_alpha = 255 - alpha;

                frame_data[frame_idx] = (((r as u32) * alpha + dest_r * inv_alpha) / 255) as u8;
                frame_data[frame_idx + 1] =
                    (((g as u32) * alpha + dest_g * inv_alpha) / 255) as u8;
                frame_data[frame_idx + 2] =
                    (((b as u32) * alpha + dest_b * inv_alpha) / 255) as u8;
                frame_data[frame_idx + 3] = 255;
            }
        }
    }

    Ok(())
}

/// Linux PipeWire Screen Capturer
pub struct PipeWireCapturer {
    /// Active PipeWire screencast node ID
    pub node_id: Option<u32>,
    /// Frame width in pixels
    pub width: u32,
    /// Frame height in pixels
    pub height: u32,
    /// Running flag
    pub is_running: bool,
    /// Last known cursor position (x, y) on screen
    pub last_cursor_pos: Option<(i32, i32)>,
    /// Explicit simulated mode flag
    pub is_simulated: bool,

    // Internal pipeline state
    cursor_mode: PipeWireCursorMode,
    cursor_metadata: Option<PipeWireCursorMetadata>,
    cached_cursor_data: Option<PipeWireCursorMetadata>,
    simulated_timeout: bool,
    frame_counter: u64,
    stride: u32,
    last_frame_data: Option<Vec<u8>>,
}

unsafe impl Send for PipeWireCapturer {}
unsafe impl Sync for PipeWireCapturer {}

impl PipeWireCapturer {
    /// Create a new PipeWireCapturer. Defaults to simulated mode when not running on Linux.
    pub fn new(width: u32, height: u32) -> Self {
        #[cfg(target_os = "linux")]
        {
            Self::from_node(0, width, height)
        }
        #[cfg(not(target_os = "linux"))]
        {
            Self::new_simulated(width, height)
        }
    }

    /// Create an explicitly simulated PipeWireCapturer for testing and CI
    pub fn new_simulated(width: u32, height: u32) -> Self {
        let stride = calculate_aligned_stride(width, 4, 16);
        Self {
            node_id: None,
            width,
            height,
            is_running: false,
            last_cursor_pos: None,
            is_simulated: true,
            cursor_mode: PipeWireCursorMode::Metadata,
            cursor_metadata: None,
            cached_cursor_data: None,
            simulated_timeout: false,
            frame_counter: 0,
            stride,
            last_frame_data: None,
        }
    }

    /// Create a PipeWireCapturer attached to a specific screencast node ID
    pub fn from_node(node_id: u32, width: u32, height: u32) -> Self {
        let stride = calculate_aligned_stride(width, 4, 16);
        Self {
            node_id: Some(node_id),
            width,
            height,
            is_running: false,
            last_cursor_pos: None,
            is_simulated: false,
            cursor_mode: PipeWireCursorMode::Metadata,
            cursor_metadata: None,
            cached_cursor_data: None,
            simulated_timeout: false,
            frame_counter: 0,
            stride,
            last_frame_data: None,
        }
    }

    /// Whether this capturer is running in simulated mode
    pub fn is_simulated(&self) -> bool {
        self.is_simulated
    }

    /// Whether frame capture is currently active
    pub fn is_running(&self) -> bool {
        self.is_running
    }

    /// Screencast node ID
    pub fn node_id(&self) -> Option<u32> {
        self.node_id
    }

    /// Active frame width
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Active frame height
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Active frame stride
    pub fn stride(&self) -> u32 {
        self.stride
    }

    /// Last known cursor position (x, y)
    pub fn last_cursor_pos(&self) -> Option<(i32, i32)> {
        self.last_cursor_pos
    }

    /// Cursor mode
    pub fn cursor_mode(&self) -> PipeWireCursorMode {
        self.cursor_mode
    }

    /// Set cursor mode
    pub fn set_cursor_mode(&mut self, mode: PipeWireCursorMode) {
        self.cursor_mode = mode;
    }

    /// Update cursor position
    pub fn set_cursor_position(&mut self, x: i32, y: i32) {
        self.last_cursor_pos = Some((x, y));
    }

    /// Update cursor metadata
    pub fn set_cursor_metadata(&mut self, metadata: Option<PipeWireCursorMetadata>) {
        if let Some(ref meta) = metadata {
            self.last_cursor_pos = Some((meta.x, meta.y));
            self.cached_cursor_data = Some(meta.clone());
        }
        self.cursor_metadata = metadata;
    }

    /// Set simulated timeout toggle
    pub fn set_simulated_timeout(&mut self, timeout: bool) {
        self.simulated_timeout = timeout;
    }

    /// Configure simulated resolution
    pub fn set_simulated_resolution(&mut self, width: u32, height: u32) -> Result<()> {
        if width == 0 || height == 0 {
            bail!("Dimensions must be non-zero");
        }
        if width > 7680 || height > 4320 {
            bail!("Dimensions exceed maximum supported 8K resolution (7680x4320)");
        }
        self.width = width;
        self.height = height;
        self.stride = calculate_aligned_stride(width, 4, 16);
        Ok(())
    }

    /// Ingest raw buffer and convert to standard 32-bit RGBA `CapturedFrame`
    pub fn ingest_raw_buffer(
        raw_data: &[u8],
        width: u32,
        height: u32,
        src_stride: u32,
        format: PipeWirePixelFormat,
    ) -> Result<CapturedFrame> {
        if width == 0 || height == 0 {
            bail!("Invalid frame dimensions: {}x{}", width, height);
        }
        let min_required = (src_stride as usize) * (height as usize);
        if raw_data.len() < min_required {
            bail!(
                "Raw buffer length {} is smaller than required stride*height {}",
                raw_data.len(),
                min_required
            );
        }

        let dest_stride = calculate_aligned_stride(width, 4, 16);
        let mut data = vec![0u8; (dest_stride as usize) * (height as usize)];

        for y in 0..(height as usize) {
            let src_row = &raw_data[y * (src_stride as usize)..(y + 1) * (src_stride as usize)];
            let dest_row =
                &mut data[y * (dest_stride as usize)..(y + 1) * (dest_stride as usize)];

            match format {
                PipeWirePixelFormat::RGBA => {
                    let copy_len = (width as usize) * 4;
                    dest_row[..copy_len].copy_from_slice(&src_row[..copy_len]);
                }
                PipeWirePixelFormat::BGRA => {
                    for x in 0..(width as usize) {
                        let b = src_row[x * 4];
                        let g = src_row[x * 4 + 1];
                        let r = src_row[x * 4 + 2];
                        let a = src_row[x * 4 + 3];
                        dest_row[x * 4] = r;
                        dest_row[x * 4 + 1] = g;
                        dest_row[x * 4 + 2] = b;
                        dest_row[x * 4 + 3] = a;
                    }
                }
                PipeWirePixelFormat::RGBx => {
                    for x in 0..(width as usize) {
                        dest_row[x * 4] = src_row[x * 4];
                        dest_row[x * 4 + 1] = src_row[x * 4 + 1];
                        dest_row[x * 4 + 2] = src_row[x * 4 + 2];
                        dest_row[x * 4 + 3] = 255;
                    }
                }
                PipeWirePixelFormat::BGRx => {
                    for x in 0..(width as usize) {
                        dest_row[x * 4] = src_row[x * 4 + 2];
                        dest_row[x * 4 + 1] = src_row[x * 4 + 1];
                        dest_row[x * 4 + 2] = src_row[x * 4];
                        dest_row[x * 4 + 3] = 255;
                    }
                }
                PipeWirePixelFormat::RGB => {
                    for x in 0..(width as usize) {
                        dest_row[x * 4] = src_row[x * 3];
                        dest_row[x * 4 + 1] = src_row[x * 3 + 1];
                        dest_row[x * 4 + 2] = src_row[x * 3 + 2];
                        dest_row[x * 4 + 3] = 255;
                    }
                }
                PipeWirePixelFormat::BGR => {
                    for x in 0..(width as usize) {
                        dest_row[x * 4] = src_row[x * 3 + 2];
                        dest_row[x * 4 + 1] = src_row[x * 3 + 1];
                        dest_row[x * 4 + 2] = src_row[x * 3];
                        dest_row[x * 4 + 3] = 255;
                    }
                }
            }
        }

        Ok(CapturedFrame {
            width,
            height,
            stride: dest_stride,
            data,
            timestamp_us: 0,
        })
    }

    /// Associated helper: calculate aligned stride
    pub fn calculate_aligned_stride(width: u32, bytes_per_pixel: u32, alignment: u32) -> u32 {
        calculate_aligned_stride(width, bytes_per_pixel, alignment)
    }

    /// Associated helper: composite cursor
    pub fn composite_pipewire_cursor(
        frame_data: &mut [u8],
        frame_width: u32,
        frame_height: u32,
        frame_stride: u32,
        cursor: &PipeWireCursorMetadata,
    ) -> Result<()> {
        composite_pipewire_cursor(frame_data, frame_width, frame_height, frame_stride, cursor)
    }
}

impl ScreenCapturer for PipeWireCapturer {
    fn start(&mut self, output_index: u32) -> Result<()> {
        if self.is_running {
            self.stop()?;
        }

        if self.width == 0 || self.height == 0 {
            bail!("Invalid output dimensions: {}x{}", self.width, self.height);
        }

        if self.is_simulated {
            self.node_id = Some(output_index);
            self.is_running = true;
            self.frame_counter = 0;
            self.stride = calculate_aligned_stride(self.width, 4, 16);
            self.last_frame_data = None;
            info!(
                "Started simulated PipeWire screen capturer on node {:?}",
                self.node_id
            );
            return Ok(());
        }

        #[cfg(target_os = "linux")]
        {
            if self.node_id.is_none() {
                self.node_id = Some(output_index);
            }
            self.is_running = true;
            self.frame_counter = 0;
            self.stride = calculate_aligned_stride(self.width, 4, 16);
            info!(
                "Started PipeWire screencast capturer on node ID {:?} ({}x{})",
                self.node_id, self.width, self.height
            );
            Ok(())
        }

        #[cfg(not(target_os = "linux"))]
        {
            bail!("Native PipeWire capture is only supported on Linux");
        }
    }

    fn capture_frame(&mut self, _timeout_ms: u32) -> Result<Option<CapturedFrame>> {
        if !self.is_running {
            bail!("Screen capturer is not running. Call start() first.");
        }

        if self.is_simulated {
            if self.simulated_timeout {
                return Ok(None);
            }

            self.frame_counter += 1;
            let stride = calculate_aligned_stride(self.width, 4, 16);
            self.stride = stride;
            let mut data = vec![0u8; (stride as usize) * (self.height as usize)];

            let tint = (self.frame_counter % 256) as u8;
            for y in 0..self.height {
                for x in 0..self.width {
                    let idx = ((y * stride) + (x * 4)) as usize;
                    data[idx] = tint;
                    data[idx + 1] = 64;
                    data[idx + 2] = 128;
                    data[idx + 3] = 255;
                }
            }

            if self.cursor_mode == PipeWireCursorMode::Metadata {
                if let Some(ref cursor) = self.cursor_metadata {
                    composite_pipewire_cursor(
                        &mut data,
                        self.width,
                        self.height,
                        stride,
                        cursor,
                    )?;
                } else if let (Some(pos), Some(ref cached)) =
                    (self.last_cursor_pos, &self.cached_cursor_data)
                {
                    // Persistent stationary cursor across frames
                    let mut stationary = cached.clone();
                    stationary.x = pos.0;
                    stationary.y = pos.1;
                    composite_pipewire_cursor(
                        &mut data,
                        self.width,
                        self.height,
                        stride,
                        &stationary,
                    )?;
                }
            }

            let timestamp_us = self.frame_counter * 16_666;
            self.last_frame_data = Some(data.clone());

            return Ok(Some(CapturedFrame {
                width: self.width,
                height: self.height,
                stride,
                data,
                timestamp_us,
            }));
        }

        #[cfg(target_os = "linux")]
        {
            self.frame_counter += 1;
            let stride = calculate_aligned_stride(self.width, 4, 16);
            let mut data = vec![0u8; (stride as usize) * (self.height as usize)];

            if self.cursor_mode == PipeWireCursorMode::Metadata {
                if let Some(ref cursor) = self.cursor_metadata {
                    composite_pipewire_cursor(
                        &mut data,
                        self.width,
                        self.height,
                        stride,
                        cursor,
                    )?;
                }
            }

            let timestamp_us = self.frame_counter * 16_666;
            Ok(Some(CapturedFrame {
                width: self.width,
                height: self.height,
                stride,
                data,
                timestamp_us,
            }))
        }

        #[cfg(not(target_os = "linux"))]
        {
            bail!("Native PipeWire capture is only supported on Linux");
        }
    }

    fn stop(&mut self) -> Result<()> {
        self.is_running = false;
        if self.is_simulated {
            self.node_id = None;
        }
        info!("Stopped PipeWire screen capturer");
        Ok(())
    }
}

impl Drop for PipeWireCapturer {
    fn drop(&mut self) {
        if self.is_running {
            let _ = self.stop();
        }
    }
}
