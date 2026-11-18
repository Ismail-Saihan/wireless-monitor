//! mDNS LAN discovery service implementation using mdns-sd crate conventions

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use anyhow::{Context, Result};
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};

use super::{DiscoveryService, PeerDevice, SERVICE_TYPE};

/// Sanitize a friendly device name into an RFC 1035/1123 compliant DNS host label
pub fn sanitize_dns_label(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();

    let trimmed = sanitized.trim_matches('-');
    if trimmed.is_empty() {
        "wireless-monitor".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Parse capability attributes from mDNS TXT property map with safe fallbacks
pub fn parse_txt_capabilities(props: &HashMap<String, String>) -> (Vec<String>, String, u32) {
    let caps = props
        .get("caps")
        .or_else(|| props.get("capabilities"))
        .map(|s| {
            s.split(',')
                .map(|item| item.trim().to_string())
                .filter(|item| !item.is_empty())
                .collect::<Vec<String>>()
        })
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| vec!["1080p".to_string()]);

    let max_res = props
        .get("res")
        .or_else(|| props.get("max_resolution"))
        .filter(|s| !s.trim().is_empty())
        .cloned()
        .unwrap_or_else(|| "1920x1080".to_string());

    let max_fps = props
        .get("fps")
        .or_else(|| props.get("max_fps"))
        .and_then(|s| s.parse::<u32>().ok())
        .filter(|&fps| fps > 0)
        .unwrap_or(60);

    (caps, max_res, max_fps)
}

/// mDNS discovery service for finding and announcing host and monitor devices
pub struct MdnsDiscovery {
    simulated: bool,
    advertising: Arc<AtomicBool>,
    browsing: Arc<AtomicBool>,
    advertised_info: Arc<Mutex<Option<(String, u16, String)>>>,
    advertised_caps: Arc<Mutex<Option<(Vec<String>, String, u32)>>>,
    registered_service_fullname: Arc<Mutex<Option<String>>>,
    discovered_peers: Arc<Mutex<Vec<PeerDevice>>>,
    browse_callback: Arc<Mutex<Option<Arc<dyn Fn(PeerDevice) + Send + Sync + 'static>>>>,
    daemon: Arc<Mutex<Option<ServiceDaemon>>>,
    browse_thread: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl MdnsDiscovery {
    /// Create a real mDNS discovery service using system network interfaces
    pub fn new() -> Result<Self> {
        let daemon = ServiceDaemon::new().context("Failed to initialize mDNS ServiceDaemon")?;
        Ok(Self {
            simulated: false,
            advertising: Arc::new(AtomicBool::new(false)),
            browsing: Arc::new(AtomicBool::new(false)),
            advertised_info: Arc::new(Mutex::new(None)),
            advertised_caps: Arc::new(Mutex::new(None)),
            registered_service_fullname: Arc::new(Mutex::new(None)),
            discovered_peers: Arc::new(Mutex::new(Vec::new())),
            browse_callback: Arc::new(Mutex::new(None)),
            daemon: Arc::new(Mutex::new(Some(daemon))),
            browse_thread: Arc::new(Mutex::new(None)),
        })
    }

    /// Create an explicit simulated mDNS discovery instance for deterministic testing
    pub fn new_simulated() -> Self {
        Self {
            simulated: true,
            advertising: Arc::new(AtomicBool::new(false)),
            browsing: Arc::new(AtomicBool::new(false)),
            advertised_info: Arc::new(Mutex::new(None)),
            advertised_caps: Arc::new(Mutex::new(None)),
            registered_service_fullname: Arc::new(Mutex::new(None)),
            discovered_peers: Arc::new(Mutex::new(Vec::new())),
            browse_callback: Arc::new(Mutex::new(None)),
            daemon: Arc::new(Mutex::new(None)),
            browse_thread: Arc::new(Mutex::new(None)),
        }
    }

    /// Whether this discovery instance is running in simulated test mode
    pub fn is_simulated(&self) -> bool {
        self.simulated
    }

    /// Check if service advertising is currently active
    pub fn is_advertising(&self) -> bool {
        self.advertising.load(Ordering::SeqCst)
    }

    /// Check if service browsing is currently active
    pub fn is_browsing(&self) -> bool {
        self.browsing.load(Ordering::SeqCst)
    }

    /// Get current advertised device info: (name, port, mode)
    pub fn get_advertised_info(&self) -> Option<(String, u16, String)> {
        if !self.is_advertising() {
            return None;
        }
        self.advertised_info.lock().ok()?.clone()
    }

    /// Get current advertised device capabilities: (caps, max_resolution, max_fps)
    pub fn get_advertised_caps(&self) -> Option<(Vec<String>, String, u32)> {
        if !self.is_advertising() {
            return None;
        }
        self.advertised_caps.lock().ok()?.clone()
    }

    /// Get current registered full service name (if advertised)
    pub fn get_registered_service_name(&self) -> Option<String> {
        self.registered_service_fullname.lock().ok()?.clone()
    }

    /// Get snapshot of discovered peers
    pub fn get_discovered_peers(&self) -> Vec<PeerDevice> {
        self.discovered_peers
            .lock()
            .map(|peers| peers.clone())
            .unwrap_or_default()
    }

    /// Alias for get_discovered_peers
    pub fn discovered_peers(&self) -> Vec<PeerDevice> {
        self.get_discovered_peers()
    }

    /// Clear tracked discovered peers
    pub fn clear_discovered_peers(&self) {
        if let Ok(mut peers) = self.discovered_peers.lock() {
            peers.clear();
        }
    }

    /// Inject a peer discovery event for testing or simulation
    pub fn simulate_peer_discovered(&self, peer: PeerDevice) -> Result<()> {
        {
            let mut peers = self
                .discovered_peers
                .lock()
                .map_err(|_| anyhow::anyhow!("Mutex poisoned"))?;
            if let Some(existing) = peers.iter_mut().find(|p| p.id == peer.id) {
                *existing = peer.clone();
            } else {
                peers.push(peer.clone());
            }
        }

        if let Ok(guard) = self.browse_callback.lock() {
            if let Some(cb) = guard.as_ref() {
                cb(peer);
            }
        }

        Ok(())
    }

    /// Helper method to inject a peer discovery event for testing or simulation
    pub fn inject_peer(&self, peer: PeerDevice) -> Result<()> {
        self.simulate_peer_discovered(peer)
    }

    /// Start advertising service on mDNS with capability metadata
    pub fn start_advertising_with_caps(
        &self,
        device_name: &str,
        port: u16,
        mode: &str,
        caps: &[String],
        max_resolution: &str,
        max_fps: u32,
    ) -> Result<()> {
        let host_label = sanitize_dns_label(device_name);
        let host_name = format!("{}.local.", host_label);

        if self.simulated {
            let mut info_guard = self
                .advertised_info
                .lock()
                .map_err(|_| anyhow::anyhow!("Mutex poisoned"))?;
            *info_guard = Some((device_name.to_string(), port, mode.to_string()));

            if let Ok(mut caps_guard) = self.advertised_caps.lock() {
                *caps_guard = Some((caps.to_vec(), max_resolution.to_string(), max_fps));
            }

            let simulated_fullname = format!("{}.{}", device_name, SERVICE_TYPE);
            if let Ok(mut reg_guard) = self.registered_service_fullname.lock() {
                *reg_guard = Some(simulated_fullname);
            }

            self.advertising.store(true, Ordering::SeqCst);
            return Ok(());
        }

        let daemon_guard = self
            .daemon
            .lock()
            .map_err(|_| anyhow::anyhow!("Mutex poisoned"))?;
        let daemon = daemon_guard
            .as_ref()
            .context("mDNS daemon not initialized")?;

        let mut properties = HashMap::new();
        properties.insert("mode".to_string(), mode.to_string());
        properties.insert("id".to_string(), device_name.to_string());
        properties.insert("caps".to_string(), caps.join(","));
        properties.insert("res".to_string(), max_resolution.to_string());
        properties.insert("fps".to_string(), max_fps.to_string());
        properties.insert("os".to_string(), std::env::consts::OS.to_string());

        let service_info = ServiceInfo::new(
            SERVICE_TYPE,
            device_name,
            &host_name,
            "",
            port,
            properties,
        )
        .context("Failed to construct ServiceInfo")?
        .enable_addr_auto();

        let fullname = service_info.get_fullname().to_string();

        daemon
            .register(service_info)
            .context("Failed to register mDNS service")?;

        if let Ok(mut reg_guard) = self.registered_service_fullname.lock() {
            *reg_guard = Some(fullname);
        }

        let mut info_guard = self
            .advertised_info
            .lock()
            .map_err(|_| anyhow::anyhow!("Mutex poisoned"))?;
        *info_guard = Some((device_name.to_string(), port, mode.to_string()));

        if let Ok(mut caps_guard) = self.advertised_caps.lock() {
            *caps_guard = Some((caps.to_vec(), max_resolution.to_string(), max_fps));
        }

        self.advertising.store(true, Ordering::SeqCst);

        Ok(())
    }
}

impl DiscoveryService for MdnsDiscovery {
    fn start_advertising(&self, device_name: &str, port: u16, mode: &str) -> Result<()> {
        let default_caps = vec!["1080p".to_string()];
        self.start_advertising_with_caps(device_name, port, mode, &default_caps, "1920x1080", 60)
    }

    fn start_advertising_with_caps(
        &self,
        device_name: &str,
        port: u16,
        mode: &str,
        caps: &[String],
        max_resolution: &str,
        max_fps: u32,
    ) -> Result<()> {
        MdnsDiscovery::start_advertising_with_caps(
            self,
            device_name,
            port,
            mode,
            caps,
            max_resolution,
            max_fps,
        )
    }

    fn start_browsing<F>(&self, on_peer_found: F) -> Result<()>
    where
        F: Fn(PeerDevice) + Send + Sync + 'static,
    {
        let cb_arc: Arc<dyn Fn(PeerDevice) + Send + Sync + 'static> = Arc::new(on_peer_found);
        {
            let mut cb_guard = self
                .browse_callback
                .lock()
                .map_err(|_| anyhow::anyhow!("Mutex poisoned"))?;
            *cb_guard = Some(Arc::clone(&cb_arc));
        }

        self.browsing.store(true, Ordering::SeqCst);

        if self.simulated {
            return Ok(());
        }

        let daemon_guard = self
            .daemon
            .lock()
            .map_err(|_| anyhow::anyhow!("Mutex poisoned"))?;
        let daemon = daemon_guard
            .as_ref()
            .context("mDNS daemon not initialized")?;

        let receiver = daemon
            .browse(SERVICE_TYPE)
            .context("Failed to start mDNS browser")?;

        let browsing_flag = Arc::clone(&self.browsing);
        let discovered_peers = Arc::clone(&self.discovered_peers);
        let callback_slot = Arc::clone(&self.browse_callback);

        let handle = std::thread::spawn(move || {
            while browsing_flag.load(Ordering::SeqCst) {
                match receiver.recv_timeout(Duration::from_millis(500)) {
                    Ok(ServiceEvent::ServiceResolved(info)) => {
                        let name = info.get_fullname().to_string();
                        let port = info.get_port();
                        let addresses = info.get_addresses();
                        let ip = addresses
                            .iter()
                            .find(|a| a.is_ipv4())
                            .or_else(|| addresses.iter().next())
                            .cloned()
                            .unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED));

                        let mode = info
                            .get_property_val_str("mode")
                            .unwrap_or("monitor")
                            .to_string();
                        let id = info
                            .get_property_val_str("id")
                            .unwrap_or(&name)
                            .to_string();

                        let mut props_map = HashMap::new();
                        for key in &["caps", "capabilities", "res", "max_resolution", "fps", "max_fps"] {
                            if let Some(val) = info.get_property_val_str(key) {
                                props_map.insert((*key).to_string(), val.to_string());
                            }
                        }
                        let (capabilities, max_resolution, max_fps) = parse_txt_capabilities(&props_map);

                        let last_seen = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs())
                            .unwrap_or(0);

                        let peer = PeerDevice {
                            id,
                            name,
                            ip,
                            port,
                            mode,
                            last_seen,
                            capabilities,
                            max_resolution,
                            max_fps,
                        };

                        if let Ok(mut peers) = discovered_peers.lock() {
                            if let Some(existing) = peers.iter_mut().find(|p| p.id == peer.id) {
                                *existing = peer.clone();
                            } else {
                                peers.push(peer.clone());
                            }
                        }

                        if let Ok(guard) = callback_slot.lock() {
                            if let Some(cb) = guard.as_ref() {
                                cb(peer);
                            }
                        }
                    }
                    Ok(_) => {}
                    Err(_) => {}
                }
            }
        });

        if let Ok(mut thread_guard) = self.browse_thread.lock() {
            *thread_guard = Some(handle);
        }

        Ok(())
    }

    fn stop(&self) -> Result<()> {
        self.advertising.store(false, Ordering::SeqCst);
        self.browsing.store(false, Ordering::SeqCst);

        if let Ok(mut info) = self.advertised_info.lock() {
            *info = None;
        }

        if let Ok(mut caps_guard) = self.advertised_caps.lock() {
            *caps_guard = None;
        }

        if let Ok(mut cb) = self.browse_callback.lock() {
            *cb = None;
        }

        if let Ok(mut thread_guard) = self.browse_thread.lock() {
            if let Some(handle) = thread_guard.take() {
                let _ = handle.join();
            }
        }

        if let Ok(mut reg_guard) = self.registered_service_fullname.lock() {
            if let Some(fullname) = reg_guard.take() {
                if !self.simulated {
                    if let Ok(mut daemon_guard) = self.daemon.lock() {
                        if let Some(daemon) = daemon_guard.as_mut() {
                            let _ = daemon.unregister(&fullname);
                        }
                    }
                }
            }
        }

        if !self.simulated {
            if let Ok(mut daemon_guard) = self.daemon.lock() {
                if let Some(daemon) = daemon_guard.as_mut() {
                    let _ = daemon.stop_browse(SERVICE_TYPE);
                }
            }
        }

        Ok(())
    }
}

impl Drop for MdnsDiscovery {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
