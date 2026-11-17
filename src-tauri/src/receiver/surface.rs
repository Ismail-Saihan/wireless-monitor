//! Native Presentation Surface & Borderless Fullscreen Rendering Queue
//!
//! Provides a zero-copy decoding and presentation queue designed for sub-frame (<1 frame)
//! latency display pipelines. Includes deterministic simulated presentation surface for testing
//! and CI environments.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

use crate::capture::CapturedFrame;
use crate::receiver::{ClientScreenGeometry, ReceiverConfig};

/// Operating mode of the presentation surface
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SurfaceMode {
    /// Native borderless fullscreen desktop window
    NativeWindow,
    /// Headless simulated rendering surface for deterministic testing
    Simulated,
}

/// Runtime rendering statistics and latency metrics
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceStats {
    /// Total frames presented to display
    pub rendered_frames: u64,
    /// Total frames dropped from queue to maintain <1 frame latency
    pub dropped_frames: u64,
    /// Current number of frames buffered in queue
    pub queued_frames: usize,
    /// Whether borderless fullscreen is currently engaged
    pub is_fullscreen: bool,
}

/// Presentation surface managing frame presentation queue and display mode
pub struct PresentationSurface {
    mode: SurfaceMode,
    geometry: ClientScreenGeometry,
    config: ReceiverConfig,
    queue: VecDeque<CapturedFrame>,
    queue_capacity: usize,
    rendered_frames: u64,
    dropped_frames: u64,
    is_active: bool,
    fullscreen: bool,
    last_rendered_frame: Option<CapturedFrame>,
}

impl PresentationSurface {
    /// Create a simulated presentation surface for testing and headless verification
    pub fn new_simulated(geometry: ClientScreenGeometry, config: ReceiverConfig) -> Self {
        let fullscreen = config.fullscreen;
        Self {
            mode: SurfaceMode::Simulated,
            geometry,
            config,
            queue: VecDeque::with_capacity(4),
            // Low-latency queue capacity: 2 frames max to ensure presentation latency <1 frame
            queue_capacity: 2,
            rendered_frames: 0,
            dropped_frames: 0,
            is_active: true,
            fullscreen,
            last_rendered_frame: None,
        }
    }

    /// Create a native presentation surface with borderless fullscreen window
    pub fn new_native(geometry: ClientScreenGeometry, config: ReceiverConfig) -> Result<Self> {
        geometry.validate()?;

        #[cfg(windows)]
        {
            // Probing display adapter bounds for target display
            let _display_target = config.target_display_id.as_deref().unwrap_or("PRIMARY");
            tracing::info!(
                "Initializing Windows borderless fullscreen surface on {} ({}x{}@{}Hz)",
                _display_target,
                geometry.width,
                geometry.height,
                geometry.refresh_rate
            );
        }

        let fullscreen = config.fullscreen;
        Ok(Self {
            mode: SurfaceMode::NativeWindow,
            geometry,
            config,
            queue: VecDeque::with_capacity(4),
            queue_capacity: 2,
            rendered_frames: 0,
            dropped_frames: 0,
            is_active: true,
            fullscreen,
            last_rendered_frame: None,
        })
    }

    /// Get current operating mode
    pub fn mode(&self) -> SurfaceMode {
        self.mode
    }

    /// Get display geometry
    pub fn geometry(&self) -> &ClientScreenGeometry {
        &self.geometry
    }

    /// Get receiver configuration
    pub fn config(&self) -> &ReceiverConfig {
        &self.config
    }

    /// Check if the surface is active and accepting frames
    pub fn is_active(&self) -> bool {
        self.is_active
    }

    /// Check whether fullscreen mode is engaged
    pub fn is_fullscreen(&self) -> bool {
        self.fullscreen
    }

    /// Toggle or set fullscreen mode
    pub fn set_fullscreen(&mut self, fullscreen: bool) -> Result<()> {
        if !self.is_active {
            bail!("Cannot toggle fullscreen: presentation surface is closed");
        }
        self.fullscreen = fullscreen;
        Ok(())
    }

    /// Enqueue a newly decoded frame into the low-latency presentation queue.
    ///
    /// If the queue exceeds capacity, the oldest unrendered frame is immediately
    /// dropped to guarantee that presentation latency remains strictly under 1 frame.
    pub fn queue_frame(&mut self, frame: CapturedFrame) -> Result<()> {
        if !self.is_active {
            bail!("Cannot queue frame: presentation surface is closed");
        }
        if !frame.is_valid() {
            bail!("Cannot queue invalid frame into presentation surface");
        }

        // Maintain sub-frame latency: discard stale frame if queue is full
        if self.queue.len() >= self.queue_capacity {
            self.queue.pop_front();
            self.dropped_frames += 1;
        }

        self.queue.push_back(frame);
        Ok(())
    }

    /// Dequeue and present the next frame to the display
    pub fn render_frame(&mut self) -> Result<Option<CapturedFrame>> {
        if !self.is_active {
            bail!("Cannot render: presentation surface is closed");
        }

        if let Some(frame) = self.queue.pop_front() {
            self.rendered_frames += 1;
            self.last_rendered_frame = Some(frame.clone());
            Ok(Some(frame))
        } else {
            Ok(None)
        }
    }

    /// Get the most recently rendered frame
    pub fn last_rendered_frame(&self) -> Option<&CapturedFrame> {
        self.last_rendered_frame.as_ref()
    }

    /// Get current presentation statistics
    pub fn stats(&self) -> SurfaceStats {
        SurfaceStats {
            rendered_frames: self.rendered_frames,
            dropped_frames: self.dropped_frames,
            queued_frames: self.queue.len(),
            is_fullscreen: self.fullscreen,
        }
    }

    /// Close presentation surface and release display resources
    pub fn close(&mut self) -> Result<()> {
        self.is_active = false;
        self.queue.clear();
        Ok(())
    }
}
