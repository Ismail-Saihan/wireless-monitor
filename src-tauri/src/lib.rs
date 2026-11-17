pub mod capture;
pub mod discovery;
pub mod display;
pub mod pairing;
pub mod receiver;
pub mod stream;

use std::sync::{Arc, Mutex};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::capture::{DxgiCapturer, ScreenCapturer};
use crate::discovery::{DiscoveryService, MdnsDiscovery, PeerDevice};
use crate::display::{DisplayConfig, VirtualDisplayManager, WindowsIddCxManager};
use crate::pairing::PinAuthManager;
use crate::receiver::MonitorReceiver;
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

/// Thread-safe central application state for Tauri IPC
#[derive(Clone)]
pub struct AppState {
    pub display_manager: Arc<Mutex<Box<dyn VirtualDisplayManager>>>,
    pub screen_capturer: Arc<Mutex<Option<Box<dyn ScreenCapturer>>>>,
    pub host_session: Arc<Mutex<Option<WebRtcHostSession>>>,
    pub discovery_service: Arc<MdnsDiscovery>,
    pub pin_auth_manager: Arc<Mutex<PinAuthManager>>,
    pub receiver: Arc<Mutex<Option<MonitorReceiver>>>,
    pub active_session: Arc<Mutex<Option<SessionStatus>>>,
    pub device_id: String,
    pub hostname: String,
    pub port: u16,
    pub simulated: bool,
}

impl AppState {
    /// Initialize production AppState with native Windows subsystem integrations
    pub fn new() -> Self {
        let hostname = std::env::var("COMPUTERNAME")
            .or_else(|_| std::env::var("HOSTNAME"))
            .unwrap_or_else(|_| "localhost".to_string());

        let discovery = MdnsDiscovery::new().unwrap_or_else(|e| {
            warn!("Failed to create native mDNS discovery: {}. Falling back to simulated.", e);
            MdnsDiscovery::new_simulated()
        });

        Self {
            display_manager: Arc::new(Mutex::new(Box::new(WindowsIddCxManager::new()))),
            screen_capturer: Arc::new(Mutex::new(Some(Box::new(DxgiCapturer::new())))),
            host_session: Arc::new(Mutex::new(None)),
            discovery_service: Arc::new(discovery),
            pin_auth_manager: Arc::new(Mutex::new(PinAuthManager::new())),
            receiver: Arc::new(Mutex::new(None)),
            active_session: Arc::new(Mutex::new(None)),
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
            active_session: Arc::new(Mutex::new(None)),
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
        self.discovery_service
            .start_advertising(&self.hostname, self.port, "host")
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
        info!("Executing AppState teardown: destroying all virtual displays");
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
                info!("Window close event detected: cleaning up virtual displays");
                app_state_clone.teardown();
            }
        })
        .invoke_handler(tauri::generate_handler![
            check_system_status,
            get_device_info,
            start_discovery,
            get_discovered_peers,
            initiate_extend_display,
            submit_pairing_pin,
            stop_session,
            install_driver,
            get_session_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
