import { invoke } from "@tauri-apps/api/core";

interface SystemStatus {
  os: string;
  hostname: string;
  is_host_ready: boolean;
  is_monitor_ready: boolean;
}

async function init() {
  try {
    const status = await invoke<SystemStatus>("check_system_status");
    const infoEl = document.getElementById("device-info");
    if (infoEl) {
      infoEl.textContent = `${status.hostname} (${status.os})`;
    }
  } catch (err) {
    console.error("Failed to fetch system status:", err);
  }
}

window.addEventListener("DOMContentLoaded", init);
