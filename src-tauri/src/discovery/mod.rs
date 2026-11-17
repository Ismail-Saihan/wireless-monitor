//! Local Area Network mDNS discovery

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::net::IpAddr;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerDevice {
    pub id: String,
    pub name: String,
    pub ip: IpAddr,
    pub port: u16,
    pub mode: String, // "host" | "monitor"
}

pub trait DiscoveryService: Send + Sync {
    fn start_advertising(&self, device_name: &str, port: u16, mode: &str) -> Result<()>;
    fn stop(&self) -> Result<()>;
}
