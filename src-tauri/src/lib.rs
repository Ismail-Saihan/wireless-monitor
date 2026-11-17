pub mod capture;
pub mod discovery;
pub mod display;
pub mod pairing;
pub mod receiver;
pub mod stream;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct SystemStatus {
    pub os: String,
    pub hostname: String,
    pub is_host_ready: bool,
    pub is_monitor_ready: bool,
}

#[tauri::command]
fn check_system_status() -> SystemStatus {
    let hostname = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "localhost".to_string());

    SystemStatus {
        os: std::env::consts::OS.to_string(),
        hostname,
        is_host_ready: true,
        is_monitor_ready: true,
    }
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![check_system_status])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
