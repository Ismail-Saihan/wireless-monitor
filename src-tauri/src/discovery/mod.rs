//! Local Area Network mDNS discovery

pub mod mdns;

pub use mdns::{sanitize_dns_label, MdnsDiscovery};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::net::IpAddr;

/// mDNS service identifier for the wireless virtual monitor system
pub const SERVICE_TYPE: &str = "_wireless-monitor._tcp.local.";

/// Network peer device discovered via mDNS
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerDevice {
    pub id: String,
    pub name: String,
    pub ip: IpAddr,
    pub port: u16,
    pub mode: String, // "host" | "monitor"
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

    /// Start browsing for peer devices on mDNS
    fn start_browsing<F>(&self, on_peer_found: F) -> Result<()>
    where
        F: Fn(PeerDevice) + Send + Sync + 'static;

    /// Stop advertising and browsing
    fn stop(&self) -> Result<()>;
}
