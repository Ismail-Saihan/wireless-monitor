import { invoke } from "@tauri-apps/api/core";
import { StatusHeader, SystemStatus, SessionStatus } from "./components/StatusHeader";
import { DeviceList, PeerDevice } from "./components/DeviceList";
import { PairingModal } from "./components/PairingModal";

export interface DeviceInfo {
  device_id: string;
  hostname: string;
  os: string;
  driver_installed: boolean;
  ip: string | null;
  port: number;
  active_session: SessionStatus | null;
}

export class App {
  private statusHeader!: StatusHeader;
  private deviceList!: DeviceList;
  private pairingModal!: PairingModal;
  public currentPeers: PeerDevice[] = [];
  public activeSession: SessionStatus | null = null;
  public deviceInfo: DeviceInfo | null = null;
  private pollInterval: number | null = null;

  public async init(): Promise<void> {
    console.log("Initializing Wireless Monitor UI...");

    // Initialize UI Components
    this.statusHeader = new StatusHeader("status-header", () => this.handleInstallDriver());
    this.deviceList = new DeviceList(
      "device-list-container",
      (peer) => this.handleExtendDisplay(peer),
      (peer) => this.handleUseAsMonitor(peer),
      () => this.refreshPeers()
    );
    this.pairingModal = new PairingModal(
      "pairing-modal-container",
      (peerId, pin) => this.handlePinSubmit(peerId, pin),
      () => {
        console.log("Pairing cancelled");
      }
    );

    // Stop session button handler
    const stopBtn = document.getElementById("btn-stop-session");
    if (stopBtn) {
      stopBtn.onclick = () => this.handleStopSession();
    }

    const checkDriverBtn = document.getElementById("btn-install-driver");
    if (checkDriverBtn) {
      checkDriverBtn.onclick = () => this.handleInstallDriver();
    }

    // Initial system status & device metadata query
    await this.refreshSystemStatus();
    await this.refreshDeviceInfo();
    await this.startDiscovery();
    await this.refreshPeers();

    // Setup periodic polling for peer updates and active session state
    this.pollInterval = window.setInterval(async () => {
      await this.refreshPeers(false);
      await this.refreshSessionStatus();
    }, 2000);
  }

  public destroy(): void {
    if (this.pollInterval !== null) {
      clearInterval(this.pollInterval);
      this.pollInterval = null;
    }
  }

  private async refreshDeviceInfo(): Promise<void> {
    try {
      this.deviceInfo = await invoke<DeviceInfo>("get_device_info");
      console.log("Device Info loaded:", this.deviceInfo);
    } catch (err) {
      console.error("Failed to query device info:", err);
    }
  }

  private async refreshSystemStatus(): Promise<void> {
    try {
      const status = await invoke<SystemStatus>("check_system_status");
      this.statusHeader.updateSystem(status);

      const driverStatusEl = document.getElementById("driver-status");
      if (driverStatusEl) {
        driverStatusEl.textContent = status.driver_installed
          ? "Indirect Display Driver (IddCx) Active"
          : "Driver not detected - click Install to setup";
        driverStatusEl.className = status.driver_installed
          ? "text-xs text-emerald-400 font-medium"
          : "text-xs text-amber-400 font-medium";
      }
    } catch (err) {
      console.error("Failed to query system status:", err);
    }
  }

  private async startDiscovery(): Promise<void> {
    try {
      await invoke("start_discovery");
      console.log("mDNS auto-discovery service started");
    } catch (err) {
      console.warn("mDNS discovery start warning:", err);
    }
  }

  private async refreshPeers(showLoading: boolean = true): Promise<void> {
    if (showLoading) {
      this.deviceList.setLoading(true);
    }
    try {
      const peers = await invoke<PeerDevice[]>("get_discovered_peers");
      this.currentPeers = peers;
      this.deviceList.setPeers(peers);
    } catch (err) {
      console.error("Failed to query discovered peers:", err);
    } finally {
      if (showLoading) {
        this.deviceList.setLoading(false);
      }
    }
  }

  private async refreshSessionStatus(): Promise<void> {
    try {
      const session = await invoke<SessionStatus | null>("get_session_status");
      this.activeSession = session;
      this.statusHeader.updateSession(session);
      this.updateActiveSessionCard(session);
    } catch (err) {
      console.error("Failed to query session status:", err);
    }
  }

  private updateActiveSessionCard(session: SessionStatus | null): void {
    const card = document.getElementById("active-session-card");
    if (!card) return;

    if (session && session.state === "active") {
      card.classList.remove("hidden");
      const peerIdEl = document.getElementById("session-peer-id");
      const resEl = document.getElementById("session-resolution");
      const latencyEl = document.getElementById("session-latency");

      if (peerIdEl) peerIdEl.textContent = session.peer_id;
      if (resEl) {
        const res = session.resolution ?? "1920x1080";
        const fps = session.fps ?? 60;
        resEl.textContent = `${res} @ ${fps} Hz`;
      }
      if (latencyEl) {
        latencyEl.textContent = `${session.latency_ms ?? 16} ms`;
      }
    } else {
      card.classList.add("hidden");
    }
  }

  private async handleExtendDisplay(peer: PeerDevice): Promise<void> {
    console.log(`Initiating extend display to ${peer.name} (${peer.id})...`);
    try {
      const session = await invoke<SessionStatus>("initiate_extend_display", {
        peerId: peer.id,
      });
      console.log("Session initiated:", session);
      this.activeSession = session;
      this.pairingModal.open(peer);
    } catch (err) {
      console.error("Failed to initiate extend display:", err);
      alert(`Failed to start session with ${peer.name}: ${err}`);
    }
  }

  private async handleUseAsMonitor(peer: PeerDevice): Promise<void> {
    console.log(`Use as monitor with ${peer.name}`);
    alert(`Connecting as monitor to ${peer.name} (${peer.ip}:${peer.port})`);
  }

  private async handlePinSubmit(peerId: string, pin: string): Promise<boolean> {
    console.log(`Submitting PIN verification for peer ${peerId}...`);
    try {
      const verified = await invoke<boolean>("submit_pairing_pin", {
        peerId,
        pin,
      });

      if (verified) {
        console.log("PIN verified successfully! Virtual display created.");
        await this.refreshSessionStatus();
        await this.refreshSystemStatus();
        return true;
      } else {
        this.pairingModal.setError("Incorrect PIN. Please check device display.");
        return false;
      }
    } catch (err: any) {
      const errorMsg = typeof err === "string" ? err : err.message || "Authentication failed";
      this.pairingModal.setError(errorMsg);
      return false;
    }
  }

  private async handleStopSession(): Promise<void> {
    try {
      await invoke("stop_session");
      console.log("Streaming session stopped, virtual displays destroyed.");
      await this.refreshSessionStatus();
      await this.refreshSystemStatus();
    } catch (err) {
      console.error("Failed to stop session:", err);
    }
  }

  private async handleInstallDriver(): Promise<void> {
    try {
      await invoke("install_driver");
      await this.refreshSystemStatus();
      alert("Driver operation complete. Status refreshed.");
    } catch (err) {
      console.error("Driver installation error:", err);
      alert(`Driver installation: ${err}`);
    }
  }
}

window.addEventListener("DOMContentLoaded", () => {
  const app = new App();
  app.init();
});
