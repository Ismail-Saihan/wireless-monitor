export interface SystemStatus {
  os: string;
  hostname: string;
  is_host_ready: boolean;
  is_monitor_ready: boolean;
  driver_installed: boolean;
}

export interface SessionStatus {
  session_id: string;
  peer_id: string;
  state: "idle" | "pairing" | "connecting" | "active" | "stopped" | "error";
  monitor_id: number | null;
  resolution: string | null;
  fps: number | null;
  latency_ms: number | null;
  pin?: string | null;
}

export interface Phase2Status {
  os: string;
  role: string;
  linux_host_ready: boolean;
  windows_receiver_ready: boolean;
  active_backend: string;
  is_streaming: boolean;
  monitor_id: number | null;
  surface_active: boolean;
  frames_captured: number;
  frames_presented: number;
}

export class StatusHeader {
  private container: HTMLElement;
  private onInstallDriverCallback?: () => void;
  private onRoleChangeCallback?: (newRole: "host" | "receiver") => void;
  private onOpenSettingsCallback?: () => void;

  constructor(
    containerId: string,
    onInstallDriver?: () => void,
    onRoleChange?: (newRole: "host" | "receiver") => void,
    onOpenSettings?: () => void
  ) {
    const el = document.getElementById(containerId);
    if (!el) {
      throw new Error(`Container #${containerId} not found`);
    }
    this.container = el;
    this.onInstallDriverCallback = onInstallDriver;
    this.onRoleChangeCallback = onRoleChange;
    this.onOpenSettingsCallback = onOpenSettings;
    this.renderInitial();
  }

  public setOnOpenSettings(callback: () => void): void {
    this.onOpenSettingsCallback = callback;
  }

  private renderInitial(): void {
    this.container.innerHTML = `
      <div class="flex items-center space-x-3.5">
        <!-- Logo Icon -->
        <div class="w-9 h-9 rounded-xl bg-gradient-to-tr from-indigo-600 to-violet-500 flex items-center justify-center font-bold text-white shadow-lg shadow-indigo-500/25 border border-white/10 shrink-0">
          <svg class="w-5 h-5 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9.75 17L9 20l-1 1h8l-1-1-.75-3M3 13h18M5 17h14a2 2 0 002-2V5a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z" />
          </svg>
        </div>
        <div>
          <div class="flex items-center space-x-2">
            <h1 class="text-sm font-semibold tracking-tight text-white">Wireless Monitor</h1>
            <span id="header-lan-badge" class="inline-flex items-center px-2 py-0.5 rounded-full text-[10px] font-medium bg-emerald-500/10 text-emerald-400 border border-emerald-500/20">
              <span class="w-1.5 h-1.5 mr-1.5 rounded-full bg-emerald-400 animate-pulse"></span>
              LAN Active
            </span>
            <span id="header-os-badge" class="inline-flex items-center px-2 py-0.5 rounded-full text-[10px] font-medium bg-indigo-500/10 text-indigo-300 border border-indigo-500/20 font-mono">
              System Ready
            </span>
          </div>
          <p id="header-device-info" class="text-[11px] text-slate-400 mt-0.5">Initializing display engine...</p>
        </div>
      </div>

      <div class="flex items-center space-x-3">
        <!-- Modern Role Switcher Segmented Control -->
        <div id="header-role-switcher" class="flex items-center bg-black/40 border border-white/10 p-0.5 rounded-xl text-xs font-medium">
          <button id="btn-role-host" class="px-3.5 py-1.5 rounded-lg transition-all duration-200 bg-gradient-to-r from-indigo-600 to-indigo-500 text-white font-medium shadow-sm">
            Host (Extend)
          </button>
          <button id="btn-role-receiver" class="px-3.5 py-1.5 rounded-lg transition-all duration-200 text-slate-400 hover:text-slate-200">
            Receiver (Monitor)
          </button>
        </div>

        <!-- Driver Status Pill -->
        <div id="header-driver-status" class="flex items-center">
          <span class="inline-flex items-center px-2.5 py-1 rounded-xl text-xs font-medium bg-white/5 text-slate-400 border border-white/5">
            <span class="w-1.5 h-1.5 mr-1.5 rounded-full bg-slate-400 animate-pulse"></span>
            Checking Driver
          </span>
        </div>

        <!-- Settings Button -->
        <button id="btn-open-settings" title="Stream & Display Settings" class="flex items-center space-x-1.5 px-3 py-1.5 bg-white/5 hover:bg-white/10 text-slate-300 hover:text-white rounded-xl border border-white/10 transition-all text-xs font-medium shadow-sm active:scale-95">
          <svg class="w-3.5 h-3.5 text-indigo-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
          </svg>
          <span class="hidden sm:inline">Settings</span>
        </button>
      </div>
    `;

    // Attach role switcher listeners
    const hostBtn = document.getElementById("btn-role-host");
    const receiverBtn = document.getElementById("btn-role-receiver");
    const settingsBtn = document.getElementById("btn-open-settings");

    if (hostBtn) {
      hostBtn.onclick = () => {
        this.setRole("host");
        this.onRoleChangeCallback?.("host");
      };
    }
    if (receiverBtn) {
      receiverBtn.onclick = () => {
        this.setRole("receiver");
        this.onRoleChangeCallback?.("receiver");
      };
    }
    if (settingsBtn) {
      settingsBtn.onclick = () => {
        this.onOpenSettingsCallback?.();
      };
    }
  }

  public setRole(role: "host" | "receiver"): void {
    const hostBtn = document.getElementById("btn-role-host");
    const receiverBtn = document.getElementById("btn-role-receiver");
    const receiverCard = document.getElementById("receiver-standby-card");

    if (role === "host") {
      if (hostBtn) {
        hostBtn.className = "px-3.5 py-1.5 rounded-lg transition-all duration-200 bg-gradient-to-r from-indigo-600 to-indigo-500 text-white font-medium shadow-sm";
      }
      if (receiverBtn) {
        receiverBtn.className = "px-3.5 py-1.5 rounded-lg transition-all duration-200 text-slate-400 hover:text-slate-200";
      }
      if (receiverCard) {
        receiverCard.classList.add("hidden");
      }
    } else {
      if (hostBtn) {
        hostBtn.className = "px-3.5 py-1.5 rounded-lg transition-all duration-200 text-slate-400 hover:text-slate-200";
      }
      if (receiverBtn) {
        receiverBtn.className = "px-3.5 py-1.5 rounded-lg transition-all duration-200 bg-gradient-to-r from-indigo-600 to-indigo-500 text-white font-medium shadow-sm";
      }
      if (receiverCard) {
        receiverCard.classList.remove("hidden");
      }
    }
  }

  public updatePhase2(phase2: Phase2Status): void {
    // 1. Update OS Badge
    const osBadge = document.getElementById("header-os-badge");
    if (osBadge) {
      const osLabel = phase2.os === "windows" ? "Windows 11/10" : phase2.os === "linux" ? "Linux Wayland" : phase2.os;
      osBadge.textContent = osLabel;
    }

    // 2. Update Role control
    const currentRole = phase2.role.toLowerCase() === "receiver" ? "receiver" : "host";
    this.setRole(currentRole);
  }

  public updateSystem(status: SystemStatus): void {
    const infoEl = document.getElementById("header-device-info");
    if (infoEl) {
      infoEl.textContent = `${status.hostname} • ${status.os.toUpperCase()} Display Engine`;
    }

    const driverEl = document.getElementById("header-driver-status");
    if (driverEl) {
      if (status.driver_installed) {
        driverEl.innerHTML = `
          <span class="inline-flex items-center px-2.5 py-1 rounded-xl text-xs font-medium bg-emerald-500/10 text-emerald-400 border border-emerald-500/20">
            <svg class="w-3.5 h-3.5 mr-1 text-emerald-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M5 13l4 4L19 7" />
            </svg>
            Driver Active
          </span>
        `;
      } else {
        driverEl.innerHTML = `
          <div class="flex items-center space-x-1.5">
            <span class="inline-flex items-center px-2 py-0.5 rounded-lg text-[11px] font-medium bg-amber-500/10 text-amber-400 border border-amber-500/20">
              Driver Needed
            </span>
            <button id="btn-install-driver-action" class="px-2.5 py-1 text-xs bg-indigo-600 hover:bg-indigo-500 text-white rounded-lg font-medium transition shadow-sm active:scale-95 flex items-center space-x-1">
              <span>Install</span>
            </button>
          </div>
        `;
        const installBtn = document.getElementById("btn-install-driver-action");
        if (installBtn && this.onInstallDriverCallback) {
          installBtn.onclick = () => this.onInstallDriverCallback?.();
        }
      }
    }
  }

  public updateSession(session: SessionStatus | null): void {
    // Handled in main.ts updateActiveSessionCard
  }
}
