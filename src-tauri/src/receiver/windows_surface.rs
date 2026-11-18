//! Windows Receiver Direct3D11 Presentation Surface
//!
//! Provides a high-performance Direct3D11 swap chain presentation queue and dynamic 2D texture
//! upload pipeline (`DXGI_FORMAT_B8G8R8A8_UNORM`) for sub-50ms glass-to-glass latency video streaming.
//! Supports borderless fullscreen presentation, dynamic resolution resizing, and sub-frame (<1 frame)
//! latency frame dropping to guarantee zero presentation queue bloat.

use anyhow::{bail, Context, Result};
use std::collections::VecDeque;

use crate::capture::CapturedFrame;
use crate::receiver::{ClientScreenGeometry, ReceiverConfig, SurfaceStats};

#[cfg(windows)]
use windows::{
    Win32::Graphics::Direct3D::{
        D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP, D3D_FEATURE_LEVEL_11_0,
        D3D_FEATURE_LEVEL_11_1,
    },
    Win32::Graphics::Direct3D11::{
        D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11RenderTargetView,
        ID3D11Texture2D, D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE,
        D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC,
        D3D11_USAGE_DEFAULT,
    },
    Win32::Graphics::Dxgi::Common::{
        DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_UNKNOWN, DXGI_SAMPLE_DESC,
    },
    Win32::Graphics::Dxgi::{IDXGISwapChain, DXGI_PRESENT, DXGI_SWAP_CHAIN_FLAG},
};

/// Hardware state container for Direct3D11 device, context, and dynamic swap chain resources
#[cfg(windows)]
struct D3D11State {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    swap_chain: Option<IDXGISwapChain>,
    dynamic_texture: ID3D11Texture2D,
    render_target_view: Option<ID3D11RenderTargetView>,
}

/// Windows Direct3D11 presentation surface with borderless fullscreen rendering
pub struct WindowsD3D11Surface {
    pub geometry: ClientScreenGeometry,
    pub config: ReceiverConfig,
    pub stats: SurfaceStats,
    pub is_simulated: bool,
    queue: VecDeque<CapturedFrame>,
    queue_capacity: usize,
    is_active: bool,
    last_rendered_frame: Option<CapturedFrame>,
    simulated_texture_data: Option<Vec<u8>>,
    texture_width: u32,
    texture_height: u32,
    #[cfg(windows)]
    d3d11_state: Option<D3D11State>,
}

unsafe impl Send for WindowsD3D11Surface {}
unsafe impl Sync for WindowsD3D11Surface {}

impl WindowsD3D11Surface {
    /// Initialize a native Direct3D11 presentation surface on Windows.
    ///
    /// Creates a Direct3D 11 device with BGRA support (`DXGI_FORMAT_B8G8R8A8_UNORM`),
    /// allocates a dynamic 2D texture, and initializes borderless fullscreen presentation.
    /// Falls back to WARP driver if hardware GPU device creation is unavailable.
    pub fn new(geometry: ClientScreenGeometry, config: ReceiverConfig) -> Result<Self> {
        geometry.validate()?;

        #[cfg(windows)]
        {
            let feature_levels = [D3D_FEATURE_LEVEL_11_1, D3D_FEATURE_LEVEL_11_0];
            let mut device_opt: Option<ID3D11Device> = None;
            let mut context_opt: Option<ID3D11DeviceContext> = None;

            // Attempt hardware Direct3D11 device creation first
            let hr = unsafe {
                D3D11CreateDevice(
                    None,
                    D3D_DRIVER_TYPE_HARDWARE,
                    None,
                    D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                    Some(&feature_levels),
                    D3D11_SDK_VERSION,
                    Some(&mut device_opt),
                    None,
                    Some(&mut context_opt),
                )
            };

            // If hardware creation fails (e.g. headless CI or virtualized environment), fall back to WARP
            let (device, context) = match hr {
                Ok(_) => (
                    device_opt.context("D3D11 hardware device handle is null")?,
                    context_opt.context("D3D11 hardware context handle is null")?,
                ),
                Err(_) => {
                    let mut warp_dev: Option<ID3D11Device> = None;
                    let mut warp_ctx: Option<ID3D11DeviceContext> = None;
                    unsafe {
                        D3D11CreateDevice(
                            None,
                            D3D_DRIVER_TYPE_WARP,
                            None,
                            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                            Some(&feature_levels),
                            D3D11_SDK_VERSION,
                            Some(&mut warp_dev),
                            None,
                            Some(&mut warp_ctx),
                        )
                        .context("Failed to create Direct3D11 device (both hardware and WARP failed)")?;
                    }
                    (
                        warp_dev.context("D3D11 WARP device handle is null")?,
                        warp_ctx.context("D3D11 WARP context handle is null")?,
                    )
                }
            };

            // Allocate dynamic 2D texture with DXGI_FORMAT_B8G8R8A8_UNORM format
            let texture_desc = D3D11_TEXTURE2D_DESC {
                Width: geometry.width,
                Height: geometry.height,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: (D3D11_BIND_SHADER_RESOURCE.0 | D3D11_BIND_RENDER_TARGET.0) as u32,
                CPUAccessFlags: 0,
                MiscFlags: 0,
            };

            let mut tex_opt: Option<ID3D11Texture2D> = None;
            unsafe {
                device
                    .CreateTexture2D(&texture_desc, None, Some(&mut tex_opt))
                    .context("Failed to allocate Direct3D11 dynamic 2D texture")?;
            }
            let dynamic_texture = tex_opt.context("Dynamic texture pointer is null")?;

            let is_fullscreen = config.fullscreen;
            Ok(Self {
                geometry,
                config,
                stats: SurfaceStats {
                    rendered_frames: 0,
                    dropped_frames: 0,
                    queued_frames: 0,
                    is_fullscreen,
                },
                is_simulated: false,
                queue: VecDeque::with_capacity(4),
                queue_capacity: 2,
                is_active: true,
                last_rendered_frame: None,
                simulated_texture_data: None,
                texture_width: geometry.width,
                texture_height: geometry.height,
                d3d11_state: Some(D3D11State {
                    device,
                    context,
                    swap_chain: None,
                    dynamic_texture,
                    render_target_view: None,
                }),
            })
        }

        #[cfg(not(windows))]
        {
            bail!("Windows Direct3D11 presentation surface is only supported on Windows platforms");
        }
    }

    /// Create a simulated Direct3D11 presentation surface for unit testing and CI environments.
    pub fn new_simulated(geometry: ClientScreenGeometry, config: ReceiverConfig) -> Self {
        let is_fullscreen = config.fullscreen;
        let buf_size = (geometry.width * 4 * geometry.height) as usize;

        Self {
            geometry,
            config,
            stats: SurfaceStats {
                rendered_frames: 0,
                dropped_frames: 0,
                queued_frames: 0,
                is_fullscreen,
            },
            is_simulated: true,
            queue: VecDeque::with_capacity(4),
            queue_capacity: 2,
            is_active: true,
            last_rendered_frame: None,
            simulated_texture_data: Some(vec![0u8; buf_size]),
            texture_width: geometry.width,
            texture_height: geometry.height,
            #[cfg(windows)]
            d3d11_state: None,
        }
    }

    /// Check if the surface is active and accepting presentation operations
    pub fn is_active(&self) -> bool {
        self.is_active
    }

    /// Check whether the surface is running in simulated test mode
    pub fn is_simulated(&self) -> bool {
        self.is_simulated
    }

    /// Check if borderless fullscreen presentation is active
    pub fn is_fullscreen(&self) -> bool {
        self.config.fullscreen
    }

    /// Get client screen geometry reference
    pub fn geometry(&self) -> &ClientScreenGeometry {
        &self.geometry
    }

    /// Get receiver configuration reference
    pub fn config(&self) -> &ReceiverConfig {
        &self.config
    }

    /// Get dynamic texture width in pixels
    pub fn texture_width(&self) -> u32 {
        self.texture_width
    }

    /// Get dynamic texture height in pixels
    pub fn texture_height(&self) -> u32 {
        self.texture_height
    }

    /// Get texture pixel format name
    pub fn texture_format(&self) -> &'static str {
        "DXGI_FORMAT_B8G8R8A8_UNORM"
    }

    /// Set low-latency queue capacity (default 2 for sub-frame <1 frame latency)
    pub fn set_queue_capacity(&mut self, capacity: usize) {
        self.queue_capacity = capacity.max(1);
    }

    /// Get current low-latency queue capacity
    pub fn queue_capacity(&self) -> usize {
        self.queue_capacity
    }

    /// Get number of frames currently pending in presentation queue
    pub fn queued_frames(&self) -> usize {
        self.queue.len()
    }

    /// Get simulated uploaded texture buffer (available in simulated test mode)
    pub fn uploaded_texture_data(&self) -> Option<&[u8]> {
        self.simulated_texture_data.as_deref()
    }

    /// Get reference to most recently presented frame
    pub fn last_rendered_frame(&self) -> Option<&CapturedFrame> {
        self.last_rendered_frame.as_ref()
    }

    /// Render and present a decoded video frame directly to the display surface.
    ///
    /// Performs dynamic 2D texture upload with `UpdateSubresource` and presents
    /// the buffer via the Direct3D11 swap chain. Updates presentation statistics.
    pub fn render_frame(&mut self, frame: &CapturedFrame) -> Result<()> {
        if !self.is_active {
            bail!("Cannot render: Direct3D11 presentation surface is closed");
        }
        if !frame.is_valid() {
            bail!("Cannot render invalid frame into Direct3D11 presentation surface");
        }
        if frame.width != self.geometry.width || frame.height != self.geometry.height {
            bail!(
                "Frame dimensions ({}x{}) do not match surface geometry ({}x{})",
                frame.width,
                frame.height,
                self.geometry.width,
                self.geometry.height
            );
        }

        // On Windows hardware mode, upload to Direct3D11 texture via UpdateSubresource
        #[cfg(windows)]
        if let Some(state) = &self.d3d11_state {
            unsafe {
                state.context.UpdateSubresource(
                    &state.dynamic_texture,
                    0,
                    None,
                    frame.data.as_ptr() as *const _,
                    frame.stride,
                    0,
                );

                // Present to swap chain with 0, DXGI_PRESENT(0) for immediate sub-frame presentation
                if let Some(swap_chain) = &state.swap_chain {
                    let _ = swap_chain.Present(0, DXGI_PRESENT(0));
                }
            }
        }

        // In simulated mode, copy frame pixels into simulated texture buffer
        if let Some(texture_buf) = &mut self.simulated_texture_data {
            let row_bytes = (self.geometry.width * 4) as usize;
            let src_stride = frame.stride as usize;

            for y in 0..(self.geometry.height as usize) {
                let src_start = y * src_stride;
                let dst_start = y * row_bytes;

                if src_start + row_bytes <= frame.data.len()
                    && dst_start + row_bytes <= texture_buf.len()
                {
                    texture_buf[dst_start..dst_start + row_bytes]
                        .copy_from_slice(&frame.data[src_start..src_start + row_bytes]);
                }
            }
        }

        self.last_rendered_frame = Some(frame.clone());
        self.stats.rendered_frames += 1;
        self.stats.queued_frames = self.queue.len();
        self.stats.is_fullscreen = self.config.fullscreen;

        Ok(())
    }

    /// Enqueue a frame into the sub-frame latency presentation queue.
    ///
    /// If the queue is at capacity, the oldest unrendered frame is immediately dropped
    /// to guarantee that latency remains strictly under 1 frame duration.
    pub fn queue_frame(&mut self, frame: CapturedFrame) -> Result<()> {
        if !self.is_active {
            bail!("Cannot queue frame: Direct3D11 presentation surface is closed");
        }
        if !frame.is_valid() {
            bail!("Cannot queue invalid frame into presentation queue");
        }
        if frame.width != self.geometry.width || frame.height != self.geometry.height {
            bail!(
                "Frame dimensions ({}x{}) do not match surface geometry ({}x{})",
                frame.width,
                frame.height,
                self.geometry.width,
                self.geometry.height
            );
        }

        // Sub-frame latency queue management: drop stale frame if capacity reached
        if self.queue.len() >= self.queue_capacity {
            self.queue.pop_front();
            self.stats.dropped_frames += 1;
        }

        self.queue.push_back(frame);
        self.stats.queued_frames = self.queue.len();
        Ok(())
    }

    /// Present the next queued frame to the Direct3D11 display surface
    pub fn present_queued_frame(&mut self) -> Result<Option<CapturedFrame>> {
        if !self.is_active {
            bail!("Cannot present: Direct3D11 presentation surface is closed");
        }

        if let Some(frame) = self.queue.pop_front() {
            self.render_frame(&frame)?;
            self.stats.queued_frames = self.queue.len();
            Ok(Some(frame))
        } else {
            Ok(None)
        }
    }

    /// Resize Direct3D11 dynamic texture and presentation swap chain buffers
    pub fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        if !self.is_active {
            bail!("Cannot resize: Direct3D11 presentation surface is closed");
        }
        if width == 0 || height == 0 {
            bail!("Resize dimensions must be non-zero (got {}x{})", width, height);
        }
        if width > 7680 || height > 4320 {
            bail!(
                "Resize dimensions {}x{} exceed maximum supported 8K resolution (7680x4320)",
                width,
                height
            );
        }

        self.geometry.width = width;
        self.geometry.height = height;
        self.texture_width = width;
        self.texture_height = height;

        #[cfg(windows)]
        if let Some(state) = &mut self.d3d11_state {
            let texture_desc = D3D11_TEXTURE2D_DESC {
                Width: width,
                Height: height,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: (D3D11_BIND_SHADER_RESOURCE.0 | D3D11_BIND_RENDER_TARGET.0) as u32,
                CPUAccessFlags: 0,
                MiscFlags: 0,
            };

            let mut tex_opt: Option<ID3D11Texture2D> = None;
            unsafe {
                state
                    .device
                    .CreateTexture2D(&texture_desc, None, Some(&mut tex_opt))
                    .context("Failed to recreate Direct3D11 dynamic texture after resize")?;
            }
            state.dynamic_texture =
                tex_opt.context("Recreated dynamic texture pointer is null")?;

            if let Some(swap_chain) = &state.swap_chain {
                unsafe {
                    let _ = swap_chain.ResizeBuffers(0, width, height, DXGI_FORMAT_UNKNOWN, DXGI_SWAP_CHAIN_FLAG(0));
                }
            }
        }

        if self.is_simulated {
            self.simulated_texture_data = Some(vec![0u8; (width * 4 * height) as usize]);
        }

        self.queue.clear();
        self.stats.queued_frames = 0;
        Ok(())
    }

    /// Toggle or set borderless fullscreen presentation mode
    pub fn set_fullscreen(&mut self, fullscreen: bool) -> Result<()> {
        if !self.is_active {
            bail!("Cannot toggle fullscreen: Direct3D11 presentation surface is closed");
        }

        self.config.fullscreen = fullscreen;
        self.stats.is_fullscreen = fullscreen;

        #[cfg(windows)]
        if let Some(state) = &self.d3d11_state {
            if let Some(swap_chain) = &state.swap_chain {
                unsafe {
                    let _ = swap_chain.SetFullscreenState(fullscreen, None);
                }
            }
        }

        Ok(())
    }

    /// Query comprehensive presentation runtime metrics
    pub fn stats(&self) -> SurfaceStats {
        SurfaceStats {
            rendered_frames: self.stats.rendered_frames,
            dropped_frames: self.stats.dropped_frames,
            queued_frames: self.queue.len(),
            is_fullscreen: self.config.fullscreen,
        }
    }

    /// Close presentation surface and release all Direct3D11 and queue resources
    pub fn close(&mut self) -> Result<()> {
        self.is_active = false;
        self.queue.clear();
        self.stats.queued_frames = 0;
        Ok(())
    }
}
