//! Local Area Network mDNS discovery

pub mod mdns;

pub use mdns::{parse_txt_capabilities, sanitize_dns_label, MdnsDiscovery};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::net::IpAddr;

/// mDNS service identifier for the wireless virtual monitor system
pub const SERVICE_TYPE: &str = "_wireless-monitor._tcp.local.";

fn default_capabilities() -> Vec<String> {
    vec!["1080p".to_string()]
}

fn default_max_resolution() -> String {
    "1920x1080".to_string()
}

fn default_max_fps() -> u32 {
    60
}

/// Network peer device discovered via mDNS
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerDevice {
    pub id: String,
    pub name: String,
    pub ip: IpAddr,
    pub port: u16,
    pub mode: String, // "host" | "monitor"
    #[serde(default)]
    pub last_seen: u64,
    #[serde(default = "default_capabilities")]
    pub capabilities: Vec<String>,
    #[serde(default = "default_max_resolution")]
    pub max_resolution: String,
    #[serde(default = "default_max_fps")]
    pub max_fps: u32,
}

impl Default for PeerDevice {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            ip: IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED),
            port: 0,
            mode: "monitor".to_string(),
            last_seen: 0,
            capabilities: default_capabilities(),
            max_resolution: default_max_resolution(),
            max_fps: default_max_fps(),
        }
    }
}

impl PeerDevice {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        ip: IpAddr,
        port: u16,
        mode: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            ip,
            port,
            mode: mode.into(),
            last_seen: 0,
            capabilities: default_capabilities(),
            max_resolution: default_max_resolution(),
            max_fps: default_max_fps(),
        }
    }

    pub fn with_capabilities(
        id: impl Into<String>,
        name: impl Into<String>,
        ip: IpAddr,
        port: u16,
        mode: impl Into<String>,
        capabilities: Vec<String>,
        max_resolution: impl Into<String>,
        max_fps: u32,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            ip,
            port,
            mode: mode.into(),
            last_seen: 0,
            capabilities,
            max_resolution: max_resolution.into(),
            max_fps,
        }
    }

    pub fn is_host(&self) -> bool {
        self.mode == "host"
    }

    pub fn is_monitor(&self) -> bool {
        self.mode == "monitor"
    }
}

/// Abstract LAN service auto-discovery trait
pub trait DiscoveryService: Send + Sync {
    /// Start advertising local service on mDNS
    fn start_advertising(&self, device_name: &str, port: u16, mode: &str) -> Result<()>;

    /// Start advertising local service on mDNS with capability attributes
    fn start_advertising_with_caps(
        &self,
        device_name: &str,
        port: u16,
        mode: &str,
        caps: &[String],
        max_resolution: &str,
        max_fps: u32,
    ) -> Result<()>;

    /// Start browsing for peer devices on mDNS
    fn start_browsing<F>(&self, on_peer_found: F) -> Result<()>
    where
        F: Fn(PeerDevice) + Send + Sync + 'static;

    /// Stop advertising and browsing
    fn stop(&self) -> Result<()>;
}
