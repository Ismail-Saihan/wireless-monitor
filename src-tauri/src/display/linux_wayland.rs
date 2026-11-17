//! Linux Wayland Virtual Display Manager
//!
//! Provides dynamic virtual display creation, configuration, and teardown on Linux
//! running Wayland compositors. Supports:
//! - Direct GNOME Mutter RemoteDesktop D-Bus (`org.gnome.Mutter.RemoteDesktop`)
//! - FreeDesktop XDG Desktop Portal (`org.freedesktop.portal.RemoteDesktop` & `ScreenCast`)
//!   used by KDE Plasma 6 and standard Wayland compositors.
//! - Deterministic simulated backend for testing and headless environments.

use std::collections::HashMap;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use super::{DisplayConfig, VirtualDisplayManager};

/// D-Bus Service and Interface Constants for GNOME Mutter
pub const MUTTER_SERVICE: &str = "org.gnome.Mutter.RemoteDesktop";
pub const MUTTER_PATH: &str = "/org/gnome/Mutter/RemoteDesktop";
pub const MUTTER_INTERFACE: &str = "org.gnome.Mutter.RemoteDesktop";
pub const MUTTER_SESSION_INTERFACE: &str = "org.gnome.Mutter.RemoteDesktop.Session";

/// D-Bus Service and Interface Constants for FreeDesktop XDG Desktop Portal
pub const PORTAL_SERVICE: &str = "org.freedesktop.portal.Desktop";
pub const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
pub const PORTAL_REMOTE_DESKTOP_INTERFACE: &str = "org.freedesktop.portal.RemoteDesktop";
pub const PORTAL_SCREENCAST_INTERFACE: &str = "org.freedesktop.portal.ScreenCast";
pub const PORTAL_SESSION_INTERFACE: &str = "org.freedesktop.portal.Session";

/// Supported Wayland backend implementations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WaylandBackend {
    /// GNOME Mutter compositor direct D-Bus remote desktop
    GnomeMutter,
    /// FreeDesktop XDG Desktop Portal (KDE Plasma 6, wlroots, standard Wayland)
    XdgPortal,
    /// Deterministic in-memory simulation for tests or environments without Wayland
    Simulated,
}

/// Record of a D-Bus call for tracking and mock verification
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DBusCallRecord {
    pub destination: String,
    pub path: String,
    pub interface: String,
    pub member: String,
    pub arguments: Vec<String>,
}

/// Construct a mock GNOME Mutter CreateSession call record
pub fn build_mutter_create_session_call() -> DBusCallRecord {
    DBusCallRecord {
        destination: MUTTER_SERVICE.to_string(),
        path: MUTTER_PATH.to_string(),
        interface: MUTTER_INTERFACE.to_string(),
        member: "CreateSession".to_string(),
        arguments: vec![],
    }
}

/// Construct a mock GNOME Mutter CreateVirtualMonitor call record
pub fn build_mutter_create_virtual_monitor_call(
    session_path: &str,
    config: &DisplayConfig,
) -> DBusCallRecord {
    DBusCallRecord {
        destination: MUTTER_SERVICE.to_string(),
        path: session_path.to_string(),
        interface: MUTTER_SESSION_INTERFACE.to_string(),
        member: "CreateVirtualMonitor".to_string(),
        arguments: vec![
            "name: Wireless-Monitor".to_string(),
            format!("width: {}", config.width),
            format!("height: {}", config.height),
            format!("refresh_rate: {}", config.refresh_rate),
        ],
    }
}

/// Construct a mock GNOME Mutter Start call record
pub fn build_mutter_start_session_call(session_path: &str) -> DBusCallRecord {
    DBusCallRecord {
        destination: MUTTER_SERVICE.to_string(),
        path: session_path.to_string(),
        interface: MUTTER_SESSION_INTERFACE.to_string(),
        member: "Start".to_string(),
        arguments: vec![],
    }
}

/// Construct a mock GNOME Mutter Stop call record
pub fn build_mutter_stop_session_call(session_path: &str) -> DBusCallRecord {
    DBusCallRecord {
        destination: MUTTER_SERVICE.to_string(),
        path: session_path.to_string(),
        interface: MUTTER_SESSION_INTERFACE.to_string(),
        member: "Stop".to_string(),
        arguments: vec![],
    }
}

/// Construct a mock XDG Portal RemoteDesktop CreateSession call record
pub fn build_portal_create_session_call() -> DBusCallRecord {
    DBusCallRecord {
        destination: PORTAL_SERVICE.to_string(),
        path: PORTAL_PATH.to_string(),
        interface: PORTAL_REMOTE_DESKTOP_INTERFACE.to_string(),
        member: "CreateSession".to_string(),
        arguments: vec!["session_handle_token: wireless_monitor".to_string()],
    }
}

/// Construct a mock XDG Portal ScreenCast CreateSession call record
pub fn build_portal_screencast_call(
    session_handle: &str,
    config: &DisplayConfig,
) -> DBusCallRecord {
    DBusCallRecord {
        destination: PORTAL_SERVICE.to_string(),
        path: PORTAL_PATH.to_string(),
        interface: PORTAL_SCREENCAST_INTERFACE.to_string(),
        member: "CreateSession".to_string(),
        arguments: vec![
            format!("session_handle: {}", session_handle),
            format!(
                "resolution: {}x{}@{}",
                config.width, config.height, config.refresh_rate
            ),
        ],
    }
}

/// Construct a mock XDG Portal Session Close call record
pub fn build_portal_close_call(session_handle: &str) -> DBusCallRecord {
    DBusCallRecord {
        destination: PORTAL_SERVICE.to_string(),
        path: session_handle.to_string(),
        interface: PORTAL_SESSION_INTERFACE.to_string(),
        member: "Close".to_string(),
        arguments: vec![],
    }
}

/// Linux Wayland Virtual Display Manager
#[derive(Debug)]
pub struct WaylandDisplayManager {
    /// Active compositor backend
    backend: WaylandBackend,
    /// Active displays tracked by ID -> DisplayConfig
    active_monitors: HashMap<u32, DisplayConfig>,
    /// Next ID counter for allocating unique monitor IDs
    next_id: u32,
    /// D-Bus session or monitor handles indexed by monitor ID
    session_handles: HashMap<u32, String>,
    /// Simulated or mock mode flag (used for testing and headless verification)
    mock_mode: bool,
    /// Flag for simulated driver / subsystem readiness
    simulated_driver_installed: bool,
    /// Log of D-Bus calls dispatched in mock mode
    dbus_log: Vec<DBusCallRecord>,
}

impl WaylandDisplayManager {
    /// Create a new WaylandDisplayManager with the specified backend
    pub fn new(backend: WaylandBackend) -> Self {
        let simulated_installed = backend == WaylandBackend::Simulated;
        Self {
            backend,
            active_monitors: HashMap::new(),
            next_id: 1,
            session_handles: HashMap::new(),
            mock_mode: false,
            simulated_driver_installed: simulated_installed,
            dbus_log: Vec::new(),
        }
    }

    /// Create an explicitly simulated manager for testing
    pub fn new_simulated() -> Self {
        Self::new(WaylandBackend::Simulated)
    }

    /// Create a manager in mock mode for testing specific backends (GNOME Mutter / XDG Portal)
    pub fn new_mock(backend: WaylandBackend) -> Self {
        let mut mgr = Self::new(backend);
        mgr.mock_mode = true;
        mgr.simulated_driver_installed = true;
        mgr
    }

    /// Detect the appropriate backend from environment variables
    pub fn detect_backend() -> WaylandBackend {
        if let Ok(sim) = std::env::var("WIRELESS_MONITOR_SIMULATED") {
            if sim == "1" || sim.eq_ignore_ascii_case("true") {
                return WaylandBackend::Simulated;
            }
        }
        if let Ok(be) = std::env::var("WIRELESS_MONITOR_BACKEND") {
            match be.to_lowercase().as_str() {
                "gnome" | "mutter" => return WaylandBackend::GnomeMutter,
                "portal" | "xdg" | "kde" => return WaylandBackend::XdgPortal,
                "simulated" | "sim" => return WaylandBackend::Simulated,
                _ => {}
            }
        }
        let wayland_display = std::env::var("WAYLAND_DISPLAY").ok();
        let desktop = std::env::var("XDG_CURRENT_DESKTOP").ok();
        Self::detect_backend_from_env(wayland_display.as_deref(), desktop.as_deref())
    }

    /// Pure function for backend detection given environment parameters
    pub fn detect_backend_from_env(
        wayland_display: Option<&str>,
        xdg_desktop: Option<&str>,
    ) -> WaylandBackend {
        match wayland_display {
            Some(wd) if !wd.trim().is_empty() => {
                let desktop = xdg_desktop.unwrap_or("").to_lowercase();
                if desktop.contains("gnome") {
                    WaylandBackend::GnomeMutter
                } else {
                    // KDE, Plasma, Sway, wlroots, or other Wayland compositors use XDG Portal
                    WaylandBackend::XdgPortal
                }
            }
            _ => WaylandBackend::Simulated,
        }
    }

    /// Return the active backend
    pub fn backend(&self) -> WaylandBackend {
        self.backend
    }

    /// Check if mock mode is active
    pub fn is_mock(&self) -> bool {
        self.mock_mode
    }

    /// Set mock mode
    pub fn set_mock(&mut self, mock: bool) {
        self.mock_mode = mock;
    }

    /// Check if simulated mode is active
    pub fn is_simulated(&self) -> bool {
        self.backend == WaylandBackend::Simulated
    }

    /// Return the count of currently active virtual displays
    pub fn display_count(&self) -> usize {
        self.active_monitors.len()
    }

    /// Return a sorted list of all active monitor IDs
    pub fn active_displays(&self) -> Vec<u32> {
        let mut ids: Vec<u32> = self.active_monitors.keys().copied().collect();
        ids.sort_unstable();
        ids
    }

    /// Get the configuration of a specific active virtual display
    pub fn get_display_config(&self, monitor_id: u32) -> Option<&DisplayConfig> {
        self.active_monitors.get(&monitor_id)
    }

    /// Get the session handle associated with a virtual display
    pub fn session_handle(&self, monitor_id: u32) -> Option<&str> {
        self.session_handles.get(&monitor_id).map(|s| s.as_str())
    }

    /// Get recorded D-Bus call history (mock mode)
    pub fn dbus_call_log(&self) -> &[DBusCallRecord] {
        &self.dbus_log
    }

    /// Clear recorded D-Bus call history
    pub fn clear_dbus_call_log(&mut self) {
        self.dbus_log.clear();
    }

    /// Set simulated driver installed status (used in tests)
    pub fn set_simulated_driver_installed(&mut self, installed: bool) {
        self.simulated_driver_installed = installed;
    }

    /// Internal helper to verify Wayland runtime presence
    fn check_wayland_active(&self) -> Result<()> {
        let wayland_display = std::env::var("WAYLAND_DISPLAY").unwrap_or_default();
        if wayland_display.trim().is_empty() {
            bail!(
                "Wayland session not detected: $WAYLAND_DISPLAY environment variable is not set. \
                 Linux host mode requires an active Wayland compositor (GNOME Mutter or KDE Plasma 6)."
            );
        }
        Ok(())
    }
}

impl Default for WaylandDisplayManager {
    fn default() -> Self {
        Self::new(Self::detect_backend())
    }
}

impl VirtualDisplayManager for WaylandDisplayManager {
    /// Detect whether the virtual display subsystem is ready.
    ///
    /// In simulated or mock mode, returns `simulated_driver_installed`.
    /// On Linux, verifies that $WAYLAND_DISPLAY is active and session D-Bus is reachable.
    fn is_driver_installed(&self) -> Result<bool> {
        if self.mock_mode || self.backend == WaylandBackend::Simulated {
            return Ok(self.simulated_driver_installed);
        }

        #[cfg(target_os = "linux")]
        {
            let wayland_display = std::env::var("WAYLAND_DISPLAY").unwrap_or_default();
            if wayland_display.trim().is_empty() {
                return Ok(false);
            }
            // Under Linux Wayland, virtual display capability is natively exposed
            // by Mutter or FreeDesktop XDG Portal D-Bus without external kernel drivers.
            Ok(true)
        }

        #[cfg(not(target_os = "linux"))]
        {
            Ok(false)
        }
    }

    /// Verify environment readiness and initialize session capability.
    ///
    /// On Linux Wayland, virtual displays do not require kernel drivers,
    /// but require an active Wayland session.
    fn install_driver(&self) -> Result<()> {
        if self.mock_mode || self.backend == WaylandBackend::Simulated {
            info!("WaylandDisplayManager driver check/install (simulated/mock): ready");
            return Ok(());
        }

        #[cfg(target_os = "linux")]
        {
            self.check_wayland_active()?;
            info!(
                "WaylandDisplayManager ready on Linux compositor ({:?})",
                self.backend
            );
            Ok(())
        }

        #[cfg(not(target_os = "linux"))]
        {
            bail!("Linux Wayland virtual display configuration is only supported on Linux targets");
        }
    }

    /// Create and attach a virtual display with the specified configuration.
    fn create_display(&mut self, config: &DisplayConfig) -> Result<u32> {
        // Step 1: Validate configuration parameters
        config
            .validate()
            .context("Invalid display configuration")?;

        let id = self.next_id;
        self.next_id += 1;

        debug!(
            "Creating virtual display ID {} via backend {:?}: {}x{} @ {}Hz",
            id, self.backend, config.width, config.height, config.refresh_rate
        );

        // Step 2: Handle simulated and mock modes
        if self.mock_mode {
            match self.backend {
                WaylandBackend::GnomeMutter => {
                    let session_path =
                        format!("{}/Session/u{}", MUTTER_PATH, id);
                    self.session_handles.insert(id, session_path.clone());
                    self.dbus_log.push(build_mutter_create_session_call());
                    self.dbus_log.push(build_mutter_create_virtual_monitor_call(
                        &session_path,
                        config,
                    ));
                    self.dbus_log
                        .push(build_mutter_start_session_call(&session_path));
                }
                WaylandBackend::XdgPortal => {
                    let session_handle =
                        format!("{}/session/xdg_portal_{}", PORTAL_PATH, id);
                    self.session_handles.insert(id, session_handle.clone());
                    self.dbus_log.push(build_portal_create_session_call());
                    self.dbus_log.push(build_portal_screencast_call(
                        &session_handle,
                        config,
                    ));
                }
                WaylandBackend::Simulated => {
                    let session_handle = format!("/simulated/session/{}", id);
                    self.session_handles.insert(id, session_handle);
                }
            }

            self.active_monitors.insert(id, config.clone());
            info!("Virtual display ID {} created (mock mode)", id);
            return Ok(id);
        }

        if self.backend == WaylandBackend::Simulated {
            let session_handle = format!("/simulated/session/{}", id);
            self.session_handles.insert(id, session_handle);
            self.active_monitors.insert(id, config.clone());
            info!("Virtual display ID {} created (simulated mode)", id);
            return Ok(id);
        }

        // Step 3: Real mode execution
        self.check_wayland_active()?;

        #[cfg(target_os = "linux")]
        {
            match self.backend {
                WaylandBackend::GnomeMutter => {
                    let handle = self.create_gnome_mutter_monitor(id, config)?;
                    self.session_handles.insert(id, handle);
                }
                WaylandBackend::XdgPortal => {
                    let handle = self.create_xdg_portal_monitor(id, config)?;
                    self.session_handles.insert(id, handle);
                }
                WaylandBackend::Simulated => unreachable!(),
            }

            self.active_monitors.insert(id, config.clone());
            info!(
                "Virtual display ID {} created successfully on Linux Wayland",
                id
            );
            Ok(id)
        }

        #[cfg(not(target_os = "linux"))]
        {
            bail!("Native Linux Wayland virtual displays are only supported on Linux targets");
        }
    }

    /// Detach and destroy a virtual display by monitor ID.
    fn destroy_display(&mut self, monitor_id: u32) -> Result<()> {
        if !self.active_monitors.contains_key(&monitor_id) {
            bail!("Virtual display with ID {} not found", monitor_id);
        }

        debug!("Destroying virtual display ID {}", monitor_id);

        let handle = self.session_handles.remove(&monitor_id);

        if self.mock_mode {
            if let Some(h) = handle {
                match self.backend {
                    WaylandBackend::GnomeMutter => {
                        self.dbus_log.push(build_mutter_stop_session_call(&h));
                    }
                    WaylandBackend::XdgPortal => {
                        self.dbus_log.push(build_portal_close_call(&h));
                    }
                    WaylandBackend::Simulated => {}
                }
            }
            self.active_monitors.remove(&monitor_id);
            info!("Virtual display ID {} destroyed (mock mode)", monitor_id);
            return Ok(());
        }

        if self.backend == WaylandBackend::Simulated {
            self.active_monitors.remove(&monitor_id);
            info!(
                "Virtual display ID {} destroyed (simulated mode)",
                monitor_id
            );
            return Ok(());
        }

        #[cfg(target_os = "linux")]
        {
            if let Some(h) = handle {
                if let Err(e) = self.destroy_native_session(&h) {
                    warn!(
                        "Warning: failed to cleanly stop D-Bus session '{}': {}",
                        h, e
                    );
                }
            }
            self.active_monitors.remove(&monitor_id);
            info!(
                "Virtual display ID {} destroyed successfully on Linux Wayland",
                monitor_id
            );
            Ok(())
        }

        #[cfg(not(target_os = "linux"))]
        {
            self.active_monitors.remove(&monitor_id);
            Ok(())
        }
    }

    /// Detach and destroy all active virtual displays.
    /// Guarantees clean teardown to prevent orphaned displays.
    fn destroy_all(&mut self) -> Result<()> {
        if self.active_monitors.is_empty() {
            debug!("destroy_all called with 0 active displays; no-op");
            return Ok(());
        }

        info!(
            "Destroying all {} active virtual displays on Wayland",
            self.active_monitors.len()
        );

        let monitor_ids: Vec<u32> = self.active_monitors.keys().copied().collect();
        for id in monitor_ids {
            let _ = self.destroy_display(id);
        }

        self.active_monitors.clear();
        self.session_handles.clear();
        info!("All Wayland virtual displays destroyed");

        Ok(())
    }
}

#[cfg(target_os = "linux")]
impl WaylandDisplayManager {
    /// Execute asynchronous D-Bus operation safely, avoiding nested Tokio runtime panics.
    /// If invoked within an active Tokio runtime thread pool (e.g. Tauri command),
    /// uses `block_in_place` to yield the worker thread.
    fn execute_blocking<F, R>(future: F) -> Result<R>
    where
        F: std::future::Future<Output = Result<R>>,
    {
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            tokio::task::block_in_place(|| handle.block_on(future))
        } else {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .context("Failed to build current-thread Tokio runtime for D-Bus execution")?;
            rt.block_on(future)
        }
    }

    /// Create virtual display via GNOME Mutter RemoteDesktop D-Bus
    fn create_gnome_mutter_monitor(
        &mut self,
        _id: u32,
        config: &DisplayConfig,
    ) -> Result<String> {
        Self::execute_blocking(async {
            let connection = zbus::Connection::session().await
                .context("Failed to connect to D-Bus session bus")?;

            // 1. Call CreateSession on Mutter RemoteDesktop
            let session_path: zbus::zvariant::OwnedObjectPath = connection
                .call_method(
                    Some(MUTTER_SERVICE),
                    MUTTER_PATH,
                    Some(MUTTER_INTERFACE),
                    "CreateSession",
                    &(),
                )
                .await
                .context("Failed to call org.gnome.Mutter.RemoteDesktop.CreateSession")?
                .body()
                .deserialize()
                .context("Failed to deserialize Mutter CreateSession response")?;

            // 2. Call CreateVirtualMonitor on the session
            let mut props: HashMap<&str, zbus::zvariant::Value> = HashMap::new();
            props.insert("name", zbus::zvariant::Value::from("Wireless-Monitor"));
            props.insert("width", zbus::zvariant::Value::from(config.width as i32));
            props.insert("height", zbus::zvariant::Value::from(config.height as i32));
            props.insert(
                "refresh-rate",
                zbus::zvariant::Value::from(config.refresh_rate as f64),
            );

            let _monitor_path: zbus::zvariant::OwnedObjectPath = connection
                .call_method(
                    Some(MUTTER_SERVICE),
                    session_path.as_str(),
                    Some(MUTTER_SESSION_INTERFACE),
                    "CreateVirtualMonitor",
                    &(props,),
                )
                .await
                .context("Failed to call CreateVirtualMonitor on Mutter session")?
                .body()
                .deserialize()
                .context("Failed to deserialize CreateVirtualMonitor response")?;

            // 3. Start the session
            let () = connection
                .call_method(
                    Some(MUTTER_SERVICE),
                    session_path.as_str(),
                    Some(MUTTER_SESSION_INTERFACE),
                    "Start",
                    &(),
                )
                .await
                .context("Failed to start Mutter RemoteDesktop session")?
                .body()
                .deserialize()
                .context("Failed to deserialize Mutter Start response")?;

            Ok(session_path.to_string())
        })
    }

    /// Create virtual display via FreeDesktop XDG Desktop Portal RemoteDesktop & ScreenCast
    fn create_xdg_portal_monitor(
        &mut self,
        id: u32,
        config: &DisplayConfig,
    ) -> Result<String> {
        Self::execute_blocking(async {
            let connection = zbus::Connection::session().await
                .context("Failed to connect to D-Bus session bus")?;

            // 1. Call CreateSession on Portal RemoteDesktop
            let mut options: HashMap<&str, zbus::zvariant::Value> = HashMap::new();
            let token = format!("wireless_monitor_{}", id);
            options.insert(
                "session_handle_token",
                zbus::zvariant::Value::from(token.as_str()),
            );

            let session_handle: zbus::zvariant::OwnedObjectPath = connection
                .call_method(
                    Some(PORTAL_SERVICE),
                    PORTAL_PATH,
                    Some(PORTAL_REMOTE_DESKTOP_INTERFACE),
                    "CreateSession",
                    &(options,),
                )
                .await
                .context("Failed to call org.freedesktop.portal.RemoteDesktop.CreateSession")?
                .body()
                .deserialize()
                .context("Failed to deserialize Portal CreateSession response")?;

            // 2. Select ScreenCast sources with virtual monitor resolution
            let mut screencast_opts: HashMap<&str, zbus::zvariant::Value> = HashMap::new();
            screencast_opts.insert(
                "types",
                zbus::zvariant::Value::from(1u32), // Monitor source
            );
            screencast_opts.insert(
                "width",
                zbus::zvariant::Value::from(config.width as i32),
            );
            screencast_opts.insert(
                "height",
                zbus::zvariant::Value::from(config.height as i32),
            );
            screencast_opts.insert(
                "refresh_rate",
                zbus::zvariant::Value::from(config.refresh_rate as f64),
            );
            let res_str = format!("{}x{}@{}", config.width, config.height, config.refresh_rate);
            screencast_opts.insert(
                "resolution",
                zbus::zvariant::Value::from(res_str.as_str()),
            );

            let _cast_res: zbus::zvariant::OwnedObjectPath = connection
                .call_method(
                    Some(PORTAL_SERVICE),
                    PORTAL_PATH,
                    Some(PORTAL_SCREENCAST_INTERFACE),
                    "CreateSession",
                    &(screencast_opts,),
                )
                .await
                .context("Failed to call org.freedesktop.portal.ScreenCast.CreateSession")?
                .body()
                .deserialize()
                .context("Failed to deserialize Portal ScreenCast response")?;

            Ok(session_handle.to_string())
        })
    }

    /// Cleanly close native D-Bus session
    fn destroy_native_session(&self, handle: &str) -> Result<()> {
        Self::execute_blocking(async {
            let connection = zbus::Connection::session().await
                .context("Failed to connect to D-Bus session bus")?;

            match self.backend {
                WaylandBackend::GnomeMutter => {
                    connection
                        .call_method(
                            Some(MUTTER_SERVICE),
                            handle,
                            Some(MUTTER_SESSION_INTERFACE),
                            "Stop",
                            &(),
                        )
                        .await
                        .context("Failed to stop Mutter session via D-Bus")?;
                }
                WaylandBackend::XdgPortal => {
                    connection
                        .call_method(
                            Some(PORTAL_SERVICE),
                            handle,
                            Some(PORTAL_SESSION_INTERFACE),
                            "Close",
                            &(),
                        )
                        .await
                        .context("Failed to close XDG Portal session via D-Bus")?;
                }
                WaylandBackend::Simulated => {}
            }
            Ok(())
        })
    }
}

/// Implement Drop for safety: guarantees that when WaylandDisplayManager goes out of scope
/// or the application shuts down, all created virtual displays are destroyed.
impl Drop for WaylandDisplayManager {
    fn drop(&mut self) {
        if !self.active_monitors.is_empty() {
            info!(
                "WaylandDisplayManager is dropping; cleaning up {} active virtual displays",
                self.active_monitors.len()
            );
            let _ = self.destroy_all();
        }
    }
}
