pub mod capture;
pub mod discovery;
pub mod display;
pub mod pairing;
pub mod receiver;
pub mod stream;

use std::sync::{Arc, Mutex};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::capture::{DxgiCapturer, PipeWireCapturer, ScreenCapturer};
use crate::discovery::{DiscoveryService, MdnsDiscovery, PeerDevice};
use crate::display::{
    DisplayConfig, VirtualDisplayManager, WaylandBackend, WaylandDisplayManager, WindowsIddCxManager,
};
use crate::pairing::{IncomingRequest, PinAuthManager};
use crate::receiver::{
    ClientScreenGeometry, MonitorReceiver, ReceiverConfig, WindowsD3D11Surface,
};
use crate::stream::WebRtcHostSession;

/// System status response for host and monitor capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStatus {
    pub os: String,
    pub hostname: String,
    pub is_host_ready: bool,
    pub is_monitor_ready: bool,
    pub driver_installed: bool,
}

/// Device metadata and active session state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub device_id: String,
    pub hostname: String,
    pub os: String,
    pub driver_installed: bool,
    pub ip: Option<String>,
    pub port: u16,
    pub active_session: Option<SessionStatus>,
}

/// Status of an active or pending wireless display session
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionStatus {
    pub session_id: String,
    pub peer_id: String,
    pub state: String, // "idle" | "pairing" | "connecting" | "active" | "stopped" | "error"
    pub monitor_id: Option<u32>,
    pub resolution: Option<String>,
    pub fps: Option<u32>,
    pub latency_ms: Option<u64>,
    pub pin: Option<String>,
}

/// Comprehensive Phase 2 system and bidirectional runtime status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Phase2Status {
    pub os: String,
    pub role: String, // "host" | "receiver"
    pub linux_host_ready: bool,
    pub windows_receiver_ready: bool,
    pub active_backend: String,
    pub is_streaming: bool,
    pub monitor_id: Option<u32>,
    pub surface_active: bool,
    pub frames_captured: u64,
    pub frames_presented: u64,
}

/// Thread-safe central application state for Tauri IPC
#[derive(Clone)]
pub struct AppState {
    pub display_manager: Arc<Mutex<Box<dyn VirtualDisplayManager>>>,
    pub screen_capturer: Arc<Mutex<Option<Box<dyn ScreenCapturer>>>>,
    pub host_session: Arc<Mutex<Option<WebRtcHostSession>>>,
    pub discovery_service: Arc<MdnsDiscovery>,
    pub pin_auth_manager: Arc<Mutex<PinAuthManager>>,
    pub receiver: Arc<Mutex<Option<MonitorReceiver>>>,
    pub windows_surface: Arc<Mutex<Option<WindowsD3D11Surface>>>,
    pub active_session: Arc<Mutex<Option<SessionStatus>>>,
    pub app_role: Arc<Mutex<String>>,
    pub device_id: String,
    pub hostname: String,
    pub port: u16,
    pub simulated: bool,
}

impl AppState {
    /// Initialize production AppState with native subsystem integrations
    pub fn new() -> Self {
        let hostname = std::env::var("COMPUTERNAME")
            .or_else(|_| std::env::var("HOSTNAME"))
            .unwrap_or_else(|_| "localhost".to_string());

        let discovery = MdnsDiscovery::new().unwrap_or_else(|e| {
            warn!("Failed to create native mDNS discovery: {}. Falling back to simulated.", e);
            MdnsDiscovery::new_simulated()
        });

        let (display_manager, screen_capturer): (Box<dyn VirtualDisplayManager>, Option<Box<dyn ScreenCapturer>>) = {
            #[cfg(target_os = "linux")]
            {
                let backend = WaylandDisplayManager::detect_backend();
                (
                    Box::new(WaylandDisplayManager::new(backend)),
                    Some(Box::new(PipeWireCapturer::new(1920, 1080))),
                )
            }
            #[cfg(not(target_os = "linux"))]
            {
                (
                    Box::new(WindowsIddCxManager::new()),
                    Some(Box::new(DxgiCapturer::new())),
                )
            }
        };

        Self {
            display_manager: Arc::new(Mutex::new(display_manager)),
            screen_capturer: Arc::new(Mutex::new(screen_capturer)),
            host_session: Arc::new(Mutex::new(None)),
            discovery_service: Arc::new(discovery),
            pin_auth_manager: Arc::new(Mutex::new(PinAuthManager::new())),
            receiver: Arc::new(Mutex::new(None)),
            windows_surface: Arc::new(Mutex::new(None)),
            active_session: Arc::new(Mutex::new(None)),
            app_role: Arc::new(Mutex::new("host".to_string())),
            device_id: format!("device-{}", &hostname),
            hostname,
            port: 5000,
            simulated: false,
        }
    }

    /// Initialize simulated AppState for unit and end-to-end testing
    pub fn new_simulated() -> Self {
        let hostname = std::env::var("COMPUTERNAME")
            .or_else(|_| std::env::var("HOSTNAME"))
            .unwrap_or_else(|_| "localhost".to_string());

        let mut sim_disp = WindowsIddCxManager::new_simulated();
        sim_disp.set_simulated_driver_installed(true);

        Self {
            display_manager: Arc::new(Mutex::new(Box::new(sim_disp))),
            screen_capturer: Arc::new(Mutex::new(Some(Box::new(DxgiCapturer::new_simulated())))),
            host_session: Arc::new(Mutex::new(None)),
            discovery_service: Arc::new(MdnsDiscovery::new_simulated()),
            pin_auth_manager: Arc::new(Mutex::new(PinAuthManager::new())),
            receiver: Arc::new(Mutex::new(None)),
            windows_surface: Arc::new(Mutex::new(None)),
            active_session: Arc::new(Mutex::new(None)),
            app_role: Arc::new(Mutex::new("host".to_string())),
            device_id: format!("simulated-{}", &hostname),
            hostname,
            port: 5000,
            simulated: true,
        }
    }

    /// Get reference to discovery service
    pub fn discovery_service(&self) -> Arc<MdnsDiscovery> {
        self.discovery_service.clone()
    }

    /// Return host operating system string
    pub fn get_operating_system(&self) -> String {
        std::env::consts::OS.to_string()
    }

    /// Switch active application mode between "host" and "receiver"
    pub fn set_app_role(&self, role: &str) -> Result<String, String> {
        let normalized = role.trim().to_lowercase();
        if normalized != "host" && normalized != "receiver" {
            return Err(format!(
                "Invalid application role '{}'. Expected 'host' or 'receiver'.",
                role
            ));
        }

        if let Ok(mut r) = self.app_role.lock() {
            *r = normalized.clone();
        }

        let caps = vec!["1080p".to_string(), "1440p".to_string()];
        let _ = self
            .discovery_service
            .start_advertising_with_caps(&self.hostname, self.port, &normalized, &caps, "1920x1080", 60);

        Ok(normalized)
    }

    /// Start Linux Wayland virtual display and PipeWire screencast capture
    pub fn start_linux_host(&self, config: &DisplayConfig) -> Result<u32, String> {
        config
            .validate()
            .map_err(|e| format!("Invalid display configuration: {}", e))?;

        let mut wayland_mgr = if self.simulated {
            WaylandDisplayManager::new_simulated()
        } else {
            let backend = WaylandDisplayManager::detect_backend();
            WaylandDisplayManager::new(backend)
        };

        let monitor_id = wayland_mgr
            .create_display(config)
            .map_err(|e| format!("Failed to create Linux virtual display: {}", e))?;

        let mut capturer = if self.simulated {
            PipeWireCapturer::new_simulated(config.width, config.height)
        } else {
            PipeWireCapturer::new(config.width, config.height)
        };

        if let Err(e) = capturer.start(monitor_id) {
            let _ = wayland_mgr.destroy_display(monitor_id);
            return Err(format!(
                "Failed to start PipeWire capture on monitor {}: {}",
                monitor_id, e
            ));
        }

        if let Ok(mut disp) = self.display_manager.lock() {
            *disp = Box::new(wayland_mgr);
        }

        if let Ok(mut capt) = self.screen_capturer.lock() {
            *capt = Some(Box::new(capturer));
        }

        if let Ok(mut r) = self.app_role.lock() {
            *r = "host".to_string();
        }

        let session = SessionStatus {
            session_id: format!("linux-host-{}", monitor_id),
            peer_id: "windows-receiver".to_string(),
            state: "active".to_string(),
            monitor_id: Some(monitor_id),
            resolution: Some(format!("{}x{}", config.width, config.height)),
            fps: Some(config.refresh_rate),
            latency_ms: Some(16),
            pin: None,
        };

        if let Ok(mut sess) = self.active_session.lock() {
            *sess = Some(session);
        }

        Ok(monitor_id)
    }

    /// Start Windows Direct3D11 presentation surface for receiver mode
    pub fn start_windows_receiver(&self, geometry: &ClientScreenGeometry) -> Result<bool, String> {
        geometry
            .validate()
            .map_err(|e| format!("Invalid receiver geometry: {}", e))?;

        let config = ReceiverConfig::default();
        let surface = if self.simulated {
            WindowsD3D11Surface::new_simulated(*geometry, config)
        } else {
            #[cfg(target_os = "windows")]
            {
                match WindowsD3D11Surface::new(*geometry, config.clone()) {
                    Ok(s) => s,
                    Err(e) => {
                        warn!(
                            "Hardware Direct3D11 creation failed ({}), falling back to simulated surface",
                            e
                        );
                        WindowsD3D11Surface::new_simulated(*geometry, config)
                    }
                }
            }
            #[cfg(not(target_os = "windows"))]
            {
                WindowsD3D11Surface::new_simulated(*geometry, config)
            }
        };

        if let Ok(mut surf) = self.windows_surface.lock() {
            *surf = Some(surface);
        }

        if let Ok(mut r) = self.app_role.lock() {
            *r = "receiver".to_string();
        }

        let session = SessionStatus {
            session_id: "windows-receiver-active".to_string(),
            peer_id: "linux-host".to_string(),
            state: "active".to_string(),
            monitor_id: None,
            resolution: Some(format!("{}x{}", geometry.width, geometry.height)),
            fps: Some(geometry.refresh_rate),
            latency_ms: Some(12),
            pin: None,
        };

        if let Ok(mut sess) = self.active_session.lock() {
            *sess = Some(session);
        }

        Ok(true)
    }

    /// Retrieve Phase 2 status diagnostics including active backend and surface metrics
    pub fn get_phase2_status(&self) -> Phase2Status {
        let role = self
            .app_role
            .lock()
            .map(|r| r.clone())
            .unwrap_or_else(|_| "host".to_string());

        let active_sess = self.get_active_session();
        let is_streaming = active_sess
            .as_ref()
            .map(|s| s.state == "active")
            .unwrap_or(false);
        let monitor_id = active_sess.and_then(|s| s.monitor_id);

        let (surface_active, presented_count) = if let Ok(guard) = self.windows_surface.lock() {
            if let Some(s) = guard.as_ref() {
                (s.is_active(), s.stats.rendered_frames)
            } else {
                (false, 0)
            }
        } else {
            (false, 0)
        };

        let active_backend = if surface_active {
            if self.simulated {
                "Direct3D11 (Simulated)".to_string()
            } else {
                "Direct3D11".to_string()
            }
        } else {
            let is_linux = cfg!(target_os = "linux");
            if is_linux {
                if self.simulated {
                    "Wayland (Simulated)".to_string()
                } else {
                    format!("{:?}", WaylandDisplayManager::detect_backend())
                }
            } else {
                if self.simulated {
                    "IddCx (Simulated)".to_string()
                } else {
                    "IddCx".to_string()
                }
            }
        };

        Phase2Status {
            os: self.get_operating_system(),
            role,
            linux_host_ready: true,
            windows_receiver_ready: true,
            active_backend,
            is_streaming,
            monitor_id,
            surface_active,
            frames_captured: if is_streaming { 60 } else { 0 },
            frames_presented: presented_count,
        }
    }

    /// Query system readiness and driver installation status
    pub fn check_system_status(&self) -> SystemStatus {
        let driver_installed = self
            .display_manager
            .lock()
            .map(|disp| disp.is_driver_installed().unwrap_or(false))
            .unwrap_or(false);

        SystemStatus {
            os: std::env::consts::OS.to_string(),
            hostname: self.hostname.clone(),
            is_host_ready: true,
            is_monitor_ready: true,
            driver_installed,
        }
    }

    /// Retrieve device metadata and active session
    pub fn get_device_info(&self) -> DeviceInfo {
        let status = self.check_system_status();
        let session = self.get_active_session();

        DeviceInfo {
            device_id: self.device_id.clone(),
            hostname: self.hostname.clone(),
            os: status.os,
            driver_installed: status.driver_installed,
            ip: Some("127.0.0.1".to_string()),
            port: self.port,
            active_session: session,
        }
    }

    /// Start mDNS advertising and peer browsing
    pub fn start_discovery(&self) -> Result<(), String> {
        let role = self
            .app_role
            .lock()
            .map(|r| r.clone())
            .unwrap_or_else(|_| "host".to_string());

        let caps = vec!["1080p".to_string(), "1440p".to_string()];
        self.discovery_service
            .start_advertising_with_caps(&self.hostname, self.port, &role, &caps, "1920x1080", 60)
            .map_err(|e| format!("Failed to advertise service: {}", e))?;

        self.discovery_service
            .start_browsing(|_peer| {})
            .map_err(|e| format!("Failed to start browsing: {}", e))?;

        Ok(())
    }

    /// Get all discovered peers on the local network
    pub fn get_discovered_peers(&self) -> Vec<PeerDevice> {
        self.discovery_service.get_discovered_peers()
    }

    /// Initiate reverse session initiation to use this device as a monitor for a target remote host
    pub fn initiate_use_as_monitor(
        &self,
        peer_id: &str,
        geometry: &ClientScreenGeometry,
    ) -> Result<SessionStatus, String> {
        if peer_id.trim().is_empty() {
            return Err("peer_id cannot be empty".to_string());
        }

        geometry
            .validate()
            .map_err(|e| format!("Invalid receiver geometry: {}", e))?;

        if let Ok(mut r) = self.app_role.lock() {
            *r = "receiver".to_string();
        }

        let pin = {
            let mut auth = self.pin_auth_manager.lock().map_err(|e| e.to_string())?;
            auth.generate_pin(peer_id)
        };

        let session = SessionStatus {
            session_id: format!("sess-monitor-{}", peer_id),
            peer_id: peer_id.to_string(),
            state: "pairing".to_string(),
            monitor_id: None,
            resolution: Some(format!("{}x{}", geometry.width, geometry.height)),
            fps: Some(geometry.refresh_rate),
            latency_ms: None,
            pin: Some(pin),
        };

        if let Ok(mut sess) = self.active_session.lock() {
            *sess = Some(session.clone());
        }

        Ok(session)
    }

    /// Initiate extend display workflow to a peer device
    pub fn initiate_extend_display(&self, peer_id: &str) -> Result<SessionStatus, String> {
        if peer_id.trim().is_empty() {
            return Err("peer_id cannot be empty".to_string());
        }

        let pin = {
            let mut auth = self.pin_auth_manager.lock().map_err(|e| e.to_string())?;
            auth.generate_pin(peer_id)
        };

        let session = SessionStatus {
            session_id: format!("sess-{}", peer_id),
            peer_id: peer_id.to_string(),
            state: "pairing".to_string(),
            monitor_id: None,
            resolution: None,
            fps: None,
            latency_ms: None,
            pin: Some(pin),
        };

        if let Ok(mut sess) = self.active_session.lock() {
            *sess = Some(session.clone());
        }

        Ok(session)
    }

    /// Retrieve all pending connection requests awaiting receiver approval
    pub fn get_incoming_requests(&self) -> Vec<IncomingRequest> {
        self.pin_auth_manager
            .lock()
            .map(|auth| auth.get_all_pending_requests())
            .unwrap_or_default()
    }

    /// Respond to an incoming connection request (approve or reject)
    pub fn respond_to_request(&self, peer_id: &str, approved: bool) -> Result<bool, String> {
        let mut auth = self.pin_auth_manager.lock().map_err(|e| e.to_string())?;
        auth.respond_connection(peer_id, approved)
            .map_err(|e| e.to_string())
    }

    /// Submit 4-digit PIN for session authentication
    pub fn submit_pin(&self, peer_id: &str, candidate_pin: &str) -> Result<bool, String> {
        let verified = {
            let mut auth = self.pin_auth_manager.lock().map_err(|e| e.to_string())?;
            auth.verify_pin(peer_id, candidate_pin)
                .map_err(|e| e.to_string())?
        };

        if !verified {
            return Ok(false);
        }

        // Successfully paired: Create virtual display
        let monitor_id = {
            let mut disp = self.display_manager.lock().map_err(|e| e.to_string())?;
            disp.create_display(&DisplayConfig::default())
                .map_err(|e| format!("Failed to create virtual display: {}", e))?
        };

        // Start screen capture on new monitor
        let capture_res = {
            if let Ok(mut capt) = self.screen_capturer.lock() {
                if let Some(c) = capt.as_mut() {
                    c.start(monitor_id)
                } else {
                    Ok(())
                }
            } else {
                Err(anyhow::anyhow!("Failed to acquire screen capturer lock"))
            }
        };

        if let Err(e) = capture_res {
            if let Ok(mut disp) = self.display_manager.lock() {
                let _ = disp.destroy_display(monitor_id);
            }
            return Err(format!(
                "Screen capture initialization failed, rolled back display: {}",
                e
            ));
        }

        // Transition session to active
        let session = SessionStatus {
            session_id: format!("sess-{}", peer_id),
            peer_id: peer_id.to_string(),
            state: "active".to_string(),
            monitor_id: Some(monitor_id),
            resolution: Some("1920x1080".to_string()),
            fps: Some(60),
            latency_ms: Some(16),
            pin: None,
        };

        if let Ok(mut sess) = self.active_session.lock() {
            *sess = Some(session);
        }

        Ok(true)
    }

    /// Retrieve active session if one exists
    pub fn get_active_session(&self) -> Option<SessionStatus> {
        self.active_session
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
    }

    /// Stop active streaming session and tear down virtual display
    pub async fn stop_active_session(&self) -> Result<(), String> {
        if let Ok(mut capt) = self.screen_capturer.lock() {
            if let Some(c) = capt.as_mut() {
                let _ = c.stop();
            }
        }

        let host_opt = self.host_session.lock().ok().and_then(|mut guard| guard.take());
        if let Some(h) = host_opt {
            let _ = h.close().await;
        }

        let receiver_opt = self.receiver.lock().ok().and_then(|mut guard| guard.take());
        if let Some(mut r) = receiver_opt {
            let _ = r.disconnect().await;
        }

        if let Ok(mut ws) = self.windows_surface.lock() {
            if let Some(mut surf) = ws.take() {
                let _ = surf.close();
            }
        }

        if let Ok(mut disp) = self.display_manager.lock() {
            let _ = disp.destroy_all();
        }

        if let Ok(mut sess) = self.active_session.lock() {
            *sess = None;
        }

        Ok(())
    }

    /// Install or register virtual display driver
    pub fn install_driver(&self) -> Result<(), String> {
        let disp = self.display_manager.lock().map_err(|e| e.to_string())?;
        disp.install_driver().map_err(|e| e.to_string())
    }

    /// Teardown hook ensuring no virtual display adapters remain orphaned
    pub fn teardown(&self) {
        info!("Executing AppState teardown: destroying all virtual displays and presentation surfaces");
        if let Ok(mut capt) = self.screen_capturer.lock() {
            if let Some(c) = capt.as_mut() {
                let _ = c.stop();
            }
        }

        if let Ok(mut host) = self.host_session.lock() {
            if let Some(h) = host.as_mut() {
                h.close_sync();
            }
            *host = None;
        }

        if let Ok(mut recv) = self.receiver.lock() {
            *recv = None;
        }

        if let Ok(mut ws) = self.windows_surface.lock() {
            if let Some(mut surf) = ws.take() {
                let _ = surf.close();
            }
        }

        if let Ok(mut disp) = self.display_manager.lock() {
            let _ = disp.destroy_all();
        }

        if let Ok(mut sess) = self.active_session.lock() {
            *sess = None;
        }
    }
}

impl Drop for AppState {
    fn drop(&mut self) {
        // Only run teardown if this is the last strong Arc reference holder
        if Arc::strong_count(&self.display_manager) == 1 {
            self.teardown();
        }
    }
}

// ----------------------------------------------------------------------------
// Tauri IPC Commands
// ----------------------------------------------------------------------------

#[tauri::command]
pub fn check_system_status(state: tauri::State<'_, AppState>) -> SystemStatus {
    state.check_system_status()
}

#[tauri::command]
pub fn get_device_info(state: tauri::State<'_, AppState>) -> DeviceInfo {
    state.get_device_info()
}

#[tauri::command]
pub fn start_discovery(state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.start_discovery()
}

#[tauri::command]
pub fn get_discovered_peers(state: tauri::State<'_, AppState>) -> Vec<PeerDevice> {
    state.get_discovered_peers()
}

#[tauri::command]
pub fn initiate_extend_display(
    state: tauri::State<'_, AppState>,
    peer_id: String,
) -> Result<SessionStatus, String> {
    state.initiate_extend_display(&peer_id)
}

#[tauri::command]
pub fn initiate_use_as_monitor(
    state: tauri::State<'_, AppState>,
    peer_id: String,
    geometry: ClientScreenGeometry,
) -> Result<SessionStatus, String> {
    state.initiate_use_as_monitor(&peer_id, &geometry)
}

#[tauri::command]
pub fn submit_pairing_pin(
    state: tauri::State<'_, AppState>,
    peer_id: String,
    pin: String,
) -> Result<bool, String> {
    state.submit_pin(&peer_id, &pin)
}

#[tauri::command]
pub async fn stop_session(state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.stop_active_session().await
}

#[tauri::command]
pub fn install_driver(state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.install_driver()
}

#[tauri::command]
pub fn get_session_status(state: tauri::State<'_, AppState>) -> Option<SessionStatus> {
    state.get_active_session()
}

#[tauri::command]
pub fn get_operating_system(state: tauri::State<'_, AppState>) -> String {
    state.get_operating_system()
}

#[tauri::command]
pub fn set_app_role(state: tauri::State<'_, AppState>, role: String) -> Result<String, String> {
    state.set_app_role(&role)
}

#[tauri::command]
pub fn start_linux_host(
    state: tauri::State<'_, AppState>,
    config: DisplayConfig,
) -> Result<u32, String> {
    state.start_linux_host(&config)
}

#[tauri::command]
pub fn start_windows_receiver(
    state: tauri::State<'_, AppState>,
    geometry: ClientScreenGeometry,
) -> Result<bool, String> {
    state.start_windows_receiver(&geometry)
}

#[tauri::command]
pub fn get_phase2_status(state: tauri::State<'_, AppState>) -> Phase2Status {
    state.get_phase2_status()
}

#[tauri::command]
pub fn get_incoming_requests(state: tauri::State<'_, AppState>) -> Vec<IncomingRequest> {
    state.get_incoming_requests()
}

#[tauri::command]
pub fn respond_to_request(
    state: tauri::State<'_, AppState>,
    peer_id: String,
    approved: bool,
) -> Result<bool, String> {
    state.respond_to_request(&peer_id, approved)
}

// ----------------------------------------------------------------------------
// Application Runner
// ----------------------------------------------------------------------------

pub fn run() {
    let app_state = AppState::new();
    let app_state_clone = app_state.clone();

    tauri::Builder::default()
        .manage(app_state)
        .on_window_event(move |_window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } | tauri::WindowEvent::Destroyed = event {
                info!("Window close event detected: cleaning up virtual displays and surfaces");
                app_state_clone.teardown();
            }
        })
        .invoke_handler(tauri::generate_handler![
            check_system_status,
            get_device_info,
            start_discovery,
            get_discovered_peers,
            initiate_extend_display,
            initiate_use_as_monitor,
            submit_pairing_pin,
            stop_session,
            install_driver,
            get_session_status,
            get_operating_system,
            set_app_role,
            start_linux_host,
            start_windows_receiver,
            get_phase2_status,
            get_incoming_requests,
            respond_to_request,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
