//! DXGI Desktop Duplication Screen Capture Pipeline (Windows)
//!
//! Captures desktop video frames from a display adapter output using
//! the DirectX Graphics Infrastructure (DXGI) Desktop Duplication API (`IDXGIOutputDuplication`).

use anyhow::{bail, Context, Result};
use tracing::{debug, error, info, warn};

use super::{CapturedFrame, ScreenCapturer};

#[cfg(windows)]
use windows::{
    core::Interface,
    Win32::Graphics::Direct3D::{
        D3D_DRIVER_TYPE_UNKNOWN, D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_11_1,
    },
    Win32::Graphics::Direct3D11::{
        D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
        D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAP_READ,
        D3D11_MAPPED_SUBRESOURCE, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
    },
    Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC},
    Win32::Graphics::Dxgi::{
        CreateDXGIFactory1, IDXGIAdapter1, IDXGIFactory1, IDXGIOutput, IDXGIOutput1,
        IDXGIOutputDuplication, IDXGIResource, DXGI_ERROR_ACCESS_LOST, DXGI_ERROR_WAIT_TIMEOUT,
        DXGI_OUTDUPL_FRAME_INFO, DXGI_OUTDUPL_POINTER_SHAPE_INFO, DXGI_OUTPUT_DESC,
    },
};

/// Pointer shape type returned by DXGI Desktop Duplication
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerShapeType {
    /// Monochrome 1-bpp AND/XOR bitmask
    Monochrome = 1,
    /// 32-bit color RGBA bitmap
    ColorRgba = 2,
    /// 32-bit color BGRA bitmap (native Windows DIB format)
    ColorBgra = 3,
    /// Masked color bitmap (alpha 0xFF = XOR, 0x00 = draw, else alpha blend)
    MaskedColor = 4,
}

/// Metadata and raw shape pixel buffer of the hardware mouse pointer
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PointerShape {
    pub shape_type: PointerShapeType,
    pub width: u32,
    pub height: u32,
    pub pitch: u32,
    pub hotspot_x: i32,
    pub hotspot_y: i32,
    pub data: Vec<u8>,
}

/// Screen position and visibility state of the mouse pointer
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PointerPosition {
    pub x: i32,
    pub y: i32,
    pub visible: bool,
}

/// Composite the mouse cursor shape onto a 32-bit RGBA frame buffer
///
/// Handles color alpha blending, monochrome 1-bpp bitmasks, and screen boundary clipping.
pub fn composite_cursor(
    frame_data: &mut [u8],
    frame_width: u32,
    frame_height: u32,
    frame_stride: u32,
    position: &PointerPosition,
    shape: &PointerShape,
) -> Result<()> {
    if !position.visible {
        return Ok(());
    }

    let dest_x = position.x - shape.hotspot_x;
    let dest_y = position.y - shape.hotspot_y;

    match shape.shape_type {
        PointerShapeType::ColorRgba | PointerShapeType::ColorBgra => {
            let is_bgra = shape.shape_type == PointerShapeType::ColorBgra;
            for sy in 0..shape.height {
                let dy = dest_y + (sy as i32);
                if dy < 0 || dy >= (frame_height as i32) {
                    continue;
                }
                for sx in 0..shape.width {
                    let dx = dest_x + (sx as i32);
                    if dx < 0 || dx >= (frame_width as i32) {
                        continue;
                    }

                    let shape_idx = ((sy as usize) * (shape.pitch as usize)) + ((sx as usize) * 4);
                    if shape_idx + 4 > shape.data.len() {
                        continue;
                    }

                    let (r, g, b, a) = if is_bgra {
                        (
                            shape.data[shape_idx + 2],
                            shape.data[shape_idx + 1],
                            shape.data[shape_idx],
                            shape.data[shape_idx + 3],
                        )
                    } else {
                        (
                            shape.data[shape_idx],
                            shape.data[shape_idx + 1],
                            shape.data[shape_idx + 2],
                            shape.data[shape_idx + 3],
                        )
                    };

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

                        frame_data[frame_idx] =
                            (((r as u32) * alpha + dest_r * inv_alpha) / 255) as u8;
                        frame_data[frame_idx + 1] =
                            (((g as u32) * alpha + dest_g * inv_alpha) / 255) as u8;
                        frame_data[frame_idx + 2] =
                            (((b as u32) * alpha + dest_b * inv_alpha) / 255) as u8;
                        frame_data[frame_idx + 3] = 255;
                    }
                }
            }
        }
        PointerShapeType::Monochrome => {
            // Monochrome cursor: buffer contains 2 stacked 1-bpp masks:
            // Top half: AND mask (height rows)
            // Bottom half: XOR mask (height rows)
            for sy in 0..shape.height {
                let dy = dest_y + (sy as i32);
                if dy < 0 || dy >= (frame_height as i32) {
                    continue;
                }

                let and_row_offset = (sy as usize) * (shape.pitch as usize);
                let xor_row_offset = ((sy + shape.height) as usize) * (shape.pitch as usize);

                for sx in 0..shape.width {
                    let dx = dest_x + (sx as i32);
                    if dx < 0 || dx >= (frame_width as i32) {
                        continue;
                    }

                    let byte_col = (sx / 8) as usize;
                    let bit_idx = 7 - (sx % 8);

                    let and_byte_idx = and_row_offset + byte_col;
                    let xor_byte_idx = xor_row_offset + byte_col;

                    if and_byte_idx >= shape.data.len() || xor_byte_idx >= shape.data.len() {
                        continue;
                    }

                    let and_bit = (shape.data[and_byte_idx] >> bit_idx) & 1;
                    let xor_bit = (shape.data[xor_byte_idx] >> bit_idx) & 1;

                    let frame_idx = ((dy as usize) * (frame_stride as usize)) + ((dx as usize) * 4);
                    if frame_idx + 4 > frame_data.len() {
                        continue;
                    }

                    if and_bit == 0 && xor_bit == 0 {
                        // Black
                        frame_data[frame_idx] = 0;
                        frame_data[frame_idx + 1] = 0;
                        frame_data[frame_idx + 2] = 0;
                        frame_data[frame_idx + 3] = 255;
                    } else if and_bit == 0 && xor_bit == 1 {
                        // White
                        frame_data[frame_idx] = 255;
                        frame_data[frame_idx + 1] = 255;
                        frame_data[frame_idx + 2] = 255;
                        frame_data[frame_idx + 3] = 255;
                    } else if and_bit == 1 && xor_bit == 0 {
                        // Transparent / Unchanged
                    } else if and_bit == 1 && xor_bit == 1 {
                        // Invert screen pixel
                        frame_data[frame_idx] = 255 - frame_data[frame_idx];
                        frame_data[frame_idx + 1] = 255 - frame_data[frame_idx + 1];
                        frame_data[frame_idx + 2] = 255 - frame_data[frame_idx + 2];
                        frame_data[frame_idx + 3] = 255;
                    }
                }
            }
        }
        PointerShapeType::MaskedColor => {
            // Masked color format
            for sy in 0..shape.height {
                let dy = dest_y + (sy as i32);
                if dy < 0 || dy >= (frame_height as i32) {
                    continue;
                }
                for sx in 0..shape.width {
                    let dx = dest_x + (sx as i32);
                    if dx < 0 || dx >= (frame_width as i32) {
                        continue;
                    }

                    let shape_idx = ((sy as usize) * (shape.pitch as usize)) + ((sx as usize) * 4);
                    if shape_idx + 4 > shape.data.len() {
                        continue;
                    }

                    let b = shape.data[shape_idx];
                    let g = shape.data[shape_idx + 1];
                    let r = shape.data[shape_idx + 2];
                    let a = shape.data[shape_idx + 3];

                    let frame_idx = ((dy as usize) * (frame_stride as usize)) + ((dx as usize) * 4);
                    if frame_idx + 4 > frame_data.len() {
                        continue;
                    }

                    if a == 0xFF {
                        frame_data[frame_idx] ^= r;
                        frame_data[frame_idx + 1] ^= g;
                        frame_data[frame_idx + 2] ^= b;
                        frame_data[frame_idx + 3] = 255;
                    } else if a == 0 {
                        frame_data[frame_idx] = r;
                        frame_data[frame_idx + 1] = g;
                        frame_data[frame_idx + 2] = b;
                        frame_data[frame_idx + 3] = 255;
                    } else {
                        let dest_r = frame_data[frame_idx] as u32;
                        let dest_g = frame_data[frame_idx + 1] as u32;
                        let dest_b = frame_data[frame_idx + 2] as u32;
                        let alpha = a as u32;
                        let inv_alpha = 255 - alpha;

                        frame_data[frame_idx] =
                            (((r as u32) * alpha + dest_r * inv_alpha) / 255) as u8;
                        frame_data[frame_idx + 1] =
                            (((g as u32) * alpha + dest_g * inv_alpha) / 255) as u8;
                        frame_data[frame_idx + 2] =
                            (((b as u32) * alpha + dest_b * inv_alpha) / 255) as u8;
                        frame_data[frame_idx + 3] = 255;
                    }
                }
            }
        }
    }

    Ok(())
}

/// Real Windows DXGI Hardware Duplication State
#[cfg(windows)]
struct RealDxgiState {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    duplication: IDXGIOutputDuplication,
    staging_texture: ID3D11Texture2D,
    width: u32,
    height: u32,
    last_mouse_update: i64,
    cached_pointer_shape: Option<PointerShape>,
}

/// DXGI Desktop Duplication Screen Capturer
pub struct DxgiCapturer {
    /// Active display adapter output index
    output_index: Option<u32>,
    /// Running flag
    is_running: bool,
    /// Explicit simulated mode flag
    simulated: bool,
    /// Simulated frame width
    simulated_width: u32,
    /// Simulated frame height
    simulated_height: u32,
    /// Simulated timeout toggle
    simulated_timeout: bool,
    /// Simulated cursor position
    simulated_cursor_position: Option<PointerPosition>,
    /// Simulated cursor shape
    simulated_cursor_shape: Option<PointerShape>,
    /// Monotonic frame counter for timestamps
    frame_counter: u64,
    /// Real DXGI hardware state (Windows only)
    #[cfg(windows)]
    dxgi_state: Option<RealDxgiState>,
}

impl DxgiCapturer {
    /// Create a new DxgiCapturer in real hardware mode.
    ///
    /// Requires a physical or virtual display attached to a DXGI adapter.
    /// Does NOT silently fall back to simulated mode if no display is found.
    /// To use mock capture for tests or headless environments, use [`DxgiCapturer::new_simulated()`].
    pub fn new() -> Self {
        Self {
            output_index: None,
            is_running: false,
            simulated: false,
            simulated_width: 1920,
            simulated_height: 1080,
            simulated_timeout: false,
            simulated_cursor_position: None,
            simulated_cursor_shape: None,
            frame_counter: 0,
            #[cfg(windows)]
            dxgi_state: None,
        }
    }

    /// Create an explicitly simulated DxgiCapturer for unit testing and CI
    pub fn new_simulated() -> Self {
        let mut capturer = Self::new();
        capturer.simulated = true;
        capturer
    }

    /// Whether this capturer is running in simulated mode
    pub fn is_simulated(&self) -> bool {
        self.simulated
    }

    /// Whether frame capture is currently active
    pub fn is_running(&self) -> bool {
        self.is_running
    }

    /// Currently active output index (if running)
    pub fn output_index(&self) -> Option<u32> {
        self.output_index
    }

    /// Configure simulated frame resolution
    pub fn set_simulated_resolution(&mut self, width: u32, height: u32) -> Result<()> {
        if width == 0 || height == 0 {
            bail!("Dimensions must be non-zero");
        }
        if width > 7680 || height > 4320 {
            bail!("Dimensions exceed maximum supported 8K resolution (7680x4320)");
        }
        self.simulated_width = width;
        self.simulated_height = height;
        Ok(())
    }

    /// Toggle simulated timeout behavior for unit testing
    pub fn set_simulated_timeout(&mut self, timeout: bool) {
        self.simulated_timeout = timeout;
    }

    /// Register simulated cursor position and shape for testing cursor compositing
    pub fn set_simulated_cursor(
        &mut self,
        position: Option<PointerPosition>,
        shape: Option<PointerShape>,
    ) {
        self.simulated_cursor_position = position;
        self.simulated_cursor_shape = shape;
    }
}

impl ScreenCapturer for DxgiCapturer {
    fn start(&mut self, output_index: u32) -> Result<()> {
        if self.is_running {
            self.stop()?;
        }

        if self.simulated {
            self.output_index = Some(output_index);
            self.is_running = true;
            info!(
                "Started simulated DXGI capturer on output index {}",
                output_index
            );
            return Ok(());
        }

        #[cfg(windows)]
        {
            // Real Windows DXGI desktop duplication initialization
            let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1()? };
            let adapter: IDXGIAdapter1 = unsafe {
                factory
                    .EnumAdapters1(0)
                    .context("No DXGI display adapter found")?
            };

            let output: IDXGIOutput = unsafe {
                adapter
                    .EnumOutputs(output_index)
                    .with_context(|| format!("DXGI output index {} not found", output_index))?
            };

            let output1: IDXGIOutput1 = output.cast()?;

            let mut device_opt: Option<ID3D11Device> = None;
            let mut context_opt: Option<ID3D11DeviceContext> = None;
            let feature_levels = [D3D_FEATURE_LEVEL_11_1, D3D_FEATURE_LEVEL_11_0];

            unsafe {
                D3D11CreateDevice(
                    &adapter,
                    D3D_DRIVER_TYPE_UNKNOWN,
                    None,
                    D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                    Some(&feature_levels),
                    D3D11_SDK_VERSION,
                    Some(&mut device_opt),
                    None,
                    Some(&mut context_opt),
                )?;
            }

            let device = device_opt.context("Failed to create D3D11Device")?;
            let context = context_opt.context("Failed to create D3D11DeviceContext")?;

            let duplication = unsafe { output1.DuplicateOutput(&device)? };

            let mut desc = DXGI_OUTPUT_DESC::default();
            unsafe { output.GetDesc(&mut desc)? };
            let width = (desc.DesktopCoordinates.right - desc.DesktopCoordinates.left) as u32;
            let height = (desc.DesktopCoordinates.bottom - desc.DesktopCoordinates.top) as u32;

            if width == 0 || height == 0 {
                bail!("Invalid output dimensions: {}x{}", width, height);
            }

            let staging_desc = D3D11_TEXTURE2D_DESC {
                Width: width,
                Height: height,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_STAGING,
                BindFlags: 0,
                CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                MiscFlags: 0,
            };

            let mut staging_opt: Option<ID3D11Texture2D> = None;
            unsafe {
                device.CreateTexture2D(&staging_desc, None, Some(&mut staging_opt))?;
            }
            let staging_texture = staging_opt.context("Failed to create D3D11 staging texture")?;

            self.dxgi_state = Some(RealDxgiState {
                device,
                context,
                duplication,
                staging_texture,
                width,
                height,
                last_mouse_update: 0,
                cached_pointer_shape: None,
            });

            self.output_index = Some(output_index);
            self.is_running = true;
            info!(
                "Started real DXGI desktop duplication on output {} ({}x{})",
                output_index, width, height
            );
            Ok(())
        }

        #[cfg(not(windows))]
        {
            bail!("Real DXGI screen capture is only supported on Windows");
        }
    }

    fn capture_frame(&mut self, timeout_ms: u32) -> Result<Option<CapturedFrame>> {
        if !self.is_running {
            bail!("Screen capturer is not running. Call start() first.");
        }

        if self.simulated {
            if self.simulated_timeout {
                return Ok(None);
            }

            self.frame_counter += 1;
            let stride = self.simulated_width * 4;
            let mut data = vec![0u8; (stride * self.simulated_height) as usize];

            // Render simulated background: gradient based on frame_counter
            let tint = (self.frame_counter % 256) as u8;
            for y in 0..self.simulated_height {
                for x in 0..self.simulated_width {
                    let idx = ((y * stride) + (x * 4)) as usize;
                    data[idx] = tint;
                    data[idx + 1] = 64;
                    data[idx + 2] = 128;
                    data[idx + 3] = 255;
                }
            }

            // Composite cursor if registered
            if let (Some(ref pos), Some(ref shape)) = (
                &self.simulated_cursor_position,
                &self.simulated_cursor_shape,
            ) {
                composite_cursor(
                    &mut data,
                    self.simulated_width,
                    self.simulated_height,
                    stride,
                    pos,
                    shape,
                )?;
            }

            let timestamp_us = self.frame_counter * 16_666;
            return Ok(Some(CapturedFrame {
                width: self.simulated_width,
                height: self.simulated_height,
                stride,
                data,
                timestamp_us,
            }));
        }

        #[cfg(windows)]
        {
            let state = self
                .dxgi_state
                .as_mut()
                .context("DXGI hardware state uninitialized")?;

            let mut frame_info = DXGI_OUTDUPL_FRAME_INFO::default();
            let mut resource_opt: Option<IDXGIResource> = None;

            let hr = unsafe {
                state.duplication.AcquireNextFrame(
                    timeout_ms,
                    &mut frame_info,
                    &mut resource_opt,
                )
            };

            if let Err(e) = hr {
                if e.code() == DXGI_ERROR_WAIT_TIMEOUT {
                    return Ok(None);
                } else if e.code() == DXGI_ERROR_ACCESS_LOST {
                    warn!("DXGI duplication access lost; teardown required");
                    self.stop()?;
                    bail!("DXGI output duplication access lost (desktop switch or mode change)");
                } else {
                    bail!("DXGI AcquireNextFrame failed with error: {:?}", e);
                }
            }

            let resource = resource_opt.context("No DXGI desktop resource returned")?;
            let desktop_texture: ID3D11Texture2D = resource.cast()?;

            // Copy GPU desktop texture to staging texture accessible by CPU
            unsafe {
                state
                    .context
                    .CopyResource(&state.staging_texture, &desktop_texture);
            }

            // Release frame lock as early as possible so display driver can continue rendering
            let _ = unsafe { state.duplication.ReleaseFrame() };

            // Map staging texture to CPU memory
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            unsafe {
                state.context.Map(
                    &state.staging_texture,
                    0,
                    D3D11_MAP_READ,
                    0,
                    Some(&mut mapped),
                )?;
            }

            let width = state.width;
            let height = state.height;
            let src_pitch = mapped.RowPitch as usize;
            let dest_stride = (width * 4) as usize;
            let mut frame_data = vec![0u8; dest_stride * (height as usize)];
            let src_ptr = mapped.pData as *const u8;

            for y in 0..(height as usize) {
                let src_row = unsafe {
                    std::slice::from_raw_parts(src_ptr.add(y * src_pitch), (width * 4) as usize)
                };
                let dest_row = &mut frame_data[y * dest_stride..(y + 1) * dest_stride];

                // Convert DXGI native BGRA to standard RGBA
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

            unsafe {
                state.context.Unmap(&state.staging_texture, 0);
            }

            // Process mouse cursor shape and compositing
            if frame_info.PointerPosition.Visible.as_bool() {
                if frame_info.PointerShapeBufferSize > 0
                    && frame_info.LastMouseUpdateTime > state.last_mouse_update
                {
                    let mut shape_info = DXGI_OUTDUPL_POINTER_SHAPE_INFO::default();
                    let mut shape_buf = vec![0u8; frame_info.PointerShapeBufferSize as usize];
                    let mut req_size = 0u32;

                    let shape_res = unsafe {
                        state.duplication.GetFramePointerShape(
                            shape_buf.len() as u32,
                            shape_buf.as_mut_ptr() as _,
                            &mut req_size,
                            &mut shape_info,
                        )
                    };

                    if shape_res.is_ok() {
                        let shape_type = match shape_info.Type {
                            1 => PointerShapeType::Monochrome,
                            2 => PointerShapeType::ColorBgra,
                            4 => PointerShapeType::MaskedColor,
                            _ => PointerShapeType::ColorBgra,
                        };

                        state.cached_pointer_shape = Some(PointerShape {
                            shape_type,
                            width: shape_info.Width,
                            height: shape_info.Height,
                            pitch: shape_info.Pitch,
                            hotspot_x: shape_info.HotSpot.x,
                            hotspot_y: shape_info.HotSpot.y,
                            data: shape_buf,
                        });
                        state.last_mouse_update = frame_info.LastMouseUpdateTime;
                    }
                }

                if let Some(ref shape) = state.cached_pointer_shape {
                    let pos = PointerPosition {
                        x: frame_info.PointerPosition.Position.x,
                        y: frame_info.PointerPosition.Position.y,
                        visible: true,
                    };
                    let _ = composite_cursor(
                        &mut frame_data,
                        width,
                        height,
                        dest_stride as u32,
                        &pos,
                        shape,
                    );
                }
            }

            self.frame_counter += 1;
            let timestamp_us = (frame_info.LastPresentTime / 10) as u64;

            Ok(Some(CapturedFrame {
                width,
                height,
                stride: dest_stride as u32,
                data: frame_data,
                timestamp_us,
            }))
        }

        #[cfg(not(windows))]
        {
            bail!("Real DXGI screen capture is only supported on Windows");
        }
    }

    fn stop(&mut self) -> Result<()> {
        self.is_running = false;
        self.output_index = None;
        #[cfg(windows)]
        {
            self.dxgi_state = None;
        }
        info!("Stopped DXGI screen capturer");
        Ok(())
    }
}

impl Drop for DxgiCapturer {
    fn drop(&mut self) {
        if self.is_running {
            let _ = self.stop();
        }
    }
}
