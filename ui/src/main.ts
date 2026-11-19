import "./style.css";
import { invoke } from "@tauri-apps/api/core";
import { StatusHeader, SystemStatus, SessionStatus, Phase2Status } from "./components/StatusHeader";
import { DeviceList, PeerDevice } from "./components/DeviceList";
import { PairingModal } from "./components/PairingModal";
import { SettingsDrawer, StreamSettings } from "./components/SettingsDrawer";
import { ApprovalModal, IncomingRequest } from "./components/ApprovalModal";
import { ConnectIpModal } from "./components/ConnectIpModal";

export interface DeviceInfo {
  device_id: string;
  hostname: string;
  os: string;
  driver_installed: boolean;
  ip: string | null;
  port: number;
  active_session: SessionStatus | null;
}

export interface DisplayConfig {
  width: number;
  height: number;
  refresh_rate: number;
}

export interface ClientScreenGeometry {
  width: number;
  height: number;
  refresh_rate: number;
}

export class App {
  private statusHeader!: StatusHeader;
  private deviceList!: DeviceList;
  private pairingModal!: PairingModal;
  private settingsDrawer!: SettingsDrawer;
  private approvalModal!: ApprovalModal;
  private connectIpModal!: ConnectIpModal;
  public currentPeers: PeerDevice[] = [];
  public activeSession: SessionStatus | null = null;
  public deviceInfo: DeviceInfo | null = null;
  public phase2Status: Phase2Status | null = null;
  public os: string = "";
  private pollInterval: number | null = null;

  public async init(): Promise<void> {
    console.log("Initializing Wireless Monitor Phase 3 UI...");

    // Query operating system
    try {
      this.os = await invoke<string>("get_operating_system");
      console.log("Operating System detected:", this.os);
      const footerOs = document.getElementById("footer-os-name");
      if (footerOs) {
        footerOs.textContent =
          this.os === "windows"
            ? "Windows Display Engine (IddCx)"
            : this.os === "linux"
            ? "Linux Display Engine (Wayland)"
            : this.os;
      }
    } catch (err) {
      console.warn("Could not query OS, falling back to client platform:", err);
    }

    // Initialize UI Components
    this.settingsDrawer = new SettingsDrawer(
      "settings-drawer-container",
      async (settings: StreamSettings) => {
        try {
          await invoke("set_stream_settings", { settings });
          console.log("Settings successfully applied to backend:", settings);
        } catch (err) {
          console.error("Failed to save stream settings to backend:", err);
          throw err;
        }
      }
    );
    await this.settingsDrawer.loadSettings();

    this.approvalModal = new ApprovalModal(
      "approval-modal-container",
      async (peerId: string, approved: boolean) => {
        await this.handleRespondToRequest(peerId, approved);
      }
    );

    this.statusHeader = new StatusHeader(
      "status-header",
      () => this.handleInstallDriver(),
      (newRole) => this.handleRoleChange(newRole),
      () => this.settingsDrawer.open()
    );

    this.connectIpModal = new ConnectIpModal(
      "connect-ip-modal-container",
      async (ip: string) => {
        return await this.handleConnectByIp(ip);
      },
      (peer: PeerDevice) => {
        this.handleExtendDisplay(peer);
      }
    );

    this.deviceList = new DeviceList(
      "device-list-container",
      (peer) => this.handleExtendDisplay(peer),
      (peer) => this.handleUseAsMonitor(peer),
      () => this.refreshPeers(),
      () => this.connectIpModal.open("10.179.0.245")
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

    const editSettingsBtn = document.getElementById("btn-edit-settings");
    if (editSettingsBtn) {
      editSettingsBtn.onclick = () => this.settingsDrawer.open();
    }

    // Initial system status, device metadata, discovery & Phase 2 status query
    await this.refreshSystemStatus();
    await this.refreshDeviceInfo();
    await this.refreshPhase2Status();
    await this.startDiscovery();
    await this.refreshPeers();

    // Setup periodic polling for peer updates, session state, Phase 2 metrics, and incoming requests
    this.pollInterval = window.setInterval(async () => {
      await this.refreshPeers(false);
      await this.refreshSessionStatus();
      await this.refreshPhase2Status();
      await this.pollIncomingRequests();
    }, 2000);
  }

  public destroy(): void {
    if (this.pollInterval !== null) {
      clearInterval(this.pollInterval);
      this.pollInterval = null;
    }
  }

  public async refreshPhase2Status(): Promise<void> {
    try {
      const p2 = await invoke<Phase2Status>("get_phase2_status");
      this.phase2Status = p2;
      this.statusHeader.updatePhase2(p2);
    } catch (err) {
      console.error("Failed to query Phase 2 status:", err);
    }
  }

  public async handleRoleChange(newRole: "host" | "receiver"): Promise<void> {
    console.log(`Switching application role to: ${newRole}`);
    try {
      const updatedRole = await invoke<string>("set_app_role", { role: newRole });
      console.log(`Role successfully changed to: ${updatedRole}`);
      await this.refreshPhase2Status();
    } catch (err) {
      console.error("Failed to set application role:", err);
      alert(`Role switch error: ${err}`);
    }
  }

  public async startLinuxHost(width: number = 1920, height: number = 1080, refreshRate: number = 60): Promise<number | null> {
    console.log(`Starting Linux Wayland Host (${width}x${height}@${refreshRate}Hz)...`);
    try {
      const monitorId = await invoke<number>("start_linux_host", {
        config: {
          width,
          height,
          refresh_rate: refreshRate,
        },
      });
      console.log(`Linux host started with monitor ID: ${monitorId}`);
      await this.refreshSessionStatus();
      await this.refreshPhase2Status();
      return monitorId;
    } catch (err) {
      console.error("Failed to start Linux host:", err);
      alert(`Start Linux Host failed: ${err}`);
      return null;
    }
  }

  public async startWindowsReceiver(width: number = 1920, height: number = 1080, refreshRate: number = 60): Promise<boolean> {
    console.log(`Starting Windows Direct3D11 Receiver (${width}x${height}@${refreshRate}Hz)...`);
    try {
      const success = await invoke<boolean>("start_windows_receiver", {
        geometry: {
          width,
          height,
          refresh_rate: refreshRate,
        },
      });
      console.log("Windows receiver started:", success);
      await this.refreshSessionStatus();
      await this.refreshPhase2Status();
      return success;
    } catch (err) {
      console.error("Failed to start Windows receiver:", err);
      alert(`Start Windows Receiver failed: ${err}`);
      return false;
    }
  }

  public async handleConnectByIp(ip: string): Promise<PeerDevice> {
    console.log(`Connecting directly to peer at IP: ${ip}...`);
    try {
      const peer = await invoke<PeerDevice>("connect_to_ip", { ip });
      console.log("Direct IP peer discovery successful:", peer);
      await this.refreshPeers(false);
      return peer;
    } catch (err) {
      console.error(`Direct IP connection to ${ip} failed:`, err);
      throw err;
    }
  }

  private async refreshDeviceInfo(): Promise<void> {
    try {
      this.deviceInfo = await invoke<DeviceInfo>("get_device_info");
      console.log("Device Info loaded:", this.deviceInfo);
      if (this.deviceInfo?.ip) {
        this.connectIpModal.setLocalIp(this.deviceInfo.ip);
        const receiverIp = document.getElementById("receiver-ip-display");
        if (receiverIp) {
          receiverIp.textContent = `${this.deviceInfo.ip}:${this.deviceInfo.port || 5000}`;
        }
      }
    } catch (err) {
      console.error("Failed to query device info:", err);
    }
  }

  private async refreshSystemStatus(): Promise<void> {
    try {
      const status = await invoke<SystemStatus>("check_system_status");
      this.statusHeader.updateSystem(status);

      const driverStatusEl = document.getElementById("driver-status");
      const checkDriverBtn = document.getElementById("btn-install-driver") as HTMLButtonElement | null;
      if (driverStatusEl) {
        const isLinux = this.os === "linux" || status.os === "linux";
        const driverLabel = isLinux
          ? "Wayland RemoteDesktop Service Active"
          : "Indirect Display Driver (IddCx) Active";
        driverStatusEl.textContent = status.driver_installed
          ? driverLabel
          : "Driver not detected - click Install to setup";
        driverStatusEl.className = status.driver_installed
          ? "text-xs text-emerald-400 font-medium"
          : "text-xs text-amber-400 font-medium";
      }

      if (checkDriverBtn) {
        if (status.driver_installed) {
          checkDriverBtn.textContent = "Driver Active";
          checkDriverBtn.className = "px-3 py-1.5 text-xs bg-slate-700 hover:bg-slate-600 text-slate-300 rounded-lg font-medium transition shadow-sm";
        } else {
          checkDriverBtn.textContent = "Install Driver (UAC)";
          checkDriverBtn.className = "px-3 py-1.5 text-xs bg-indigo-600 hover:bg-indigo-500 text-white rounded-lg font-medium transition shadow-md shadow-indigo-500/20";
        }
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

  private async pollIncomingRequests(): Promise<void> {
    try {
      const requests = await invoke<IncomingRequest[]>("get_incoming_requests");
      if (requests && requests.length > 0) {
        const latestRequest = requests[0];
        if (!this.approvalModal.isOpen() || this.approvalModal.getCurrentPeerId() !== latestRequest.peer_id) {
          this.approvalModal.open(latestRequest);
        }
      } else if (this.approvalModal.isOpen()) {
        this.approvalModal.close();
      }
    } catch (err) {
      console.error("Failed to query incoming requests:", err);
    }
  }

  private async handleRespondToRequest(peerId: string, approved: boolean): Promise<void> {
    console.log(`Responding to incoming request from ${peerId}: approved=${approved}`);
    try {
      const result = await invoke<boolean>("respond_to_request", {
        peerId,
        approved,
      });
      console.log(`Response result: ${result}`);
      this.approvalModal.close();
      await this.refreshSessionStatus();
      await this.refreshPhase2Status();
    } catch (err) {
      console.error("Failed to respond to request:", err);
      alert(`Failed to respond to request: ${err}`);
      throw err;
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
      this.pairingModal.open(peer, session.pin);
    } catch (err) {
      console.error("Failed to initiate extend display:", err);
      alert(`Failed to start session with ${peer.name}: ${err}`);
    }
  }

  private async handleUseAsMonitor(peer: PeerDevice): Promise<void> {
    console.log(`Initiating reverse monitor connection with ${peer.name} (${peer.id})...`);
    const rawWidth = window.screen.width || 1920;
    const rawHeight = window.screen.height || 1080;
    const screenWidth = Math.max(640, Math.floor(rawWidth / 2) * 2);
    const screenHeight = Math.max(480, Math.floor(rawHeight / 2) * 2);
    const refreshRate = 60;
    const geometry: ClientScreenGeometry = {
      width: screenWidth,
      height: screenHeight,
      refresh_rate: refreshRate,
    };

    try {
      const session = await invoke<SessionStatus>("initiate_use_as_monitor", {
        peerId: peer.id,
        geometry,
      });
      console.log("Reverse monitor session initiated:", session);
      this.activeSession = session;
      this.pairingModal.open(peer, session.pin);
    } catch (err) {
      console.error("Failed to initiate reverse monitor session:", err);
      alert(`Failed to connect as monitor to ${peer.name}: ${err}`);
    }
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
        await this.refreshPhase2Status();
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
      await this.refreshPhase2Status();
    } catch (err) {
      console.error("Failed to stop session:", err);
    }
  }

  private async handleInstallDriver(): Promise<void> {
    const checkDriverBtn = document.getElementById("btn-install-driver") as HTMLButtonElement | null;
    const actionDriverBtn = document.getElementById("btn-install-driver-action") as HTMLButtonElement | null;

    const prevCardText = checkDriverBtn ? checkDriverBtn.textContent : null;
    const prevActionText = actionDriverBtn ? actionDriverBtn.textContent : null;

    if (checkDriverBtn) {
      checkDriverBtn.textContent = "Installing (check UAC)...";
      checkDriverBtn.disabled = true;
    }
    if (actionDriverBtn) {
      actionDriverBtn.textContent = "Installing...";
      actionDriverBtn.disabled = true;
    }

    try {
      await invoke("install_driver");
      await this.refreshSystemStatus();
      await this.refreshPhase2Status();
      await this.refreshDeviceInfo();
      alert("Virtual Display Driver operation complete. Status refreshed.");
    } catch (err: any) {
      const errStr = String(err || "");
      if (errStr.includes("exit code: 1") || errStr.includes("cancelled") || errStr.includes("canceled")) {
        console.warn("Driver installation was cancelled by user.");
        alert("Driver installation was cancelled.");
      } else {
        console.error("Driver installation error:", err);
        alert(`Driver installation: ${err}`);
      }
    } finally {
      if (checkDriverBtn) {
        checkDriverBtn.disabled = false;
        if (prevCardText) checkDriverBtn.textContent = prevCardText;
      }
      if (actionDriverBtn) {
        actionDriverBtn.disabled = false;
        if (prevActionText) actionDriverBtn.textContent = prevActionText;
      }
    }
  }
}

window.addEventListener("DOMContentLoaded", () => {
  const app = new App();
  app.init();
});
