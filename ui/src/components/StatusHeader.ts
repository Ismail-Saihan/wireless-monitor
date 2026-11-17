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

export class StatusHeader {
  private container: HTMLElement;
  private onInstallDriverCallback?: () => void;

  constructor(containerId: string, onInstallDriver?: () => void) {
    const el = document.getElementById(containerId);
    if (!el) {
      throw new Error(`Container #${containerId} not found`);
    }
    this.container = el;
    this.onInstallDriverCallback = onInstallDriver;
    this.renderInitial();
  }

  private renderInitial(): void {
    this.container.innerHTML = `
      <div class="flex items-center space-x-3">
        <div class="w-9 h-9 rounded-xl bg-indigo-600 flex items-center justify-center font-bold text-white shadow-lg shadow-indigo-500/30 tracking-tight">
          WM
        </div>
        <div>
          <div class="flex items-center space-x-2">
            <h1 class="text-base font-semibold tracking-wide text-white">Wireless Monitor</h1>
            <span id="header-lan-badge" class="inline-flex items-center px-2 py-0.5 rounded-full text-[10px] font-medium bg-emerald-500/10 text-emerald-400 border border-emerald-500/20">
              <span class="w-1.5 h-1.5 mr-1.5 rounded-full bg-emerald-400 animate-pulse"></span>
              LAN Ready
            </span>
          </div>
          <p id="header-device-info" class="text-xs text-slate-400 mt-0.5">Detecting host system...</p>
        </div>
      </div>
      <div class="flex items-center space-x-3">
        <div id="header-session-pill" class="hidden"></div>
        <div id="header-driver-status" class="flex items-center space-x-2">
          <span class="text-xs text-slate-400">Driver: Checking...</span>
        </div>
      </div>
    `;
  }

  public updateSystem(status: SystemStatus): void {
    const infoEl = document.getElementById("header-device-info");
    if (infoEl) {
      infoEl.textContent = `${status.hostname} • ${status.os.toUpperCase()} Host Engine`;
    }

    const driverEl = document.getElementById("header-driver-status");
    if (driverEl) {
      if (status.driver_installed) {
        driverEl.innerHTML = `
          <span class="inline-flex items-center px-2.5 py-1 rounded-lg text-xs font-medium bg-slate-800 text-slate-300 border border-slate-700/80">
            <span class="w-1.5 h-1.5 mr-1.5 rounded-full bg-emerald-400"></span>
            IddCx Driver Ready
          </span>
        `;
      } else {
        driverEl.innerHTML = `
          <span class="inline-flex items-center px-2.5 py-1 rounded-lg text-xs font-medium bg-amber-500/10 text-amber-400 border border-amber-500/20">
            Driver Not Installed
          </span>
          <button id="btn-install-driver-action" class="px-2.5 py-1 text-xs bg-indigo-600 hover:bg-indigo-500 text-white rounded-lg font-medium transition shadow-sm">
            Install
          </button>
        `;
        const installBtn = document.getElementById("btn-install-driver-action");
        if (installBtn && this.onInstallDriverCallback) {
          installBtn.onclick = () => this.onInstallDriverCallback?.();
        }
      }
    }
  }

  public updateSession(session: SessionStatus | null): void {
    const pill = document.getElementById("header-session-pill");
    if (!pill) return;

    if (session && session.state === "active") {
      pill.classList.remove("hidden");
      const latency = session.latency_ms ?? 16;
      const res = session.resolution ?? "1920x1080";
      const fps = session.fps ?? 60;
      pill.innerHTML = `
        <div class="flex items-center space-x-2 px-3 py-1 bg-indigo-500/10 border border-indigo-500/30 rounded-xl">
          <span class="w-2 h-2 rounded-full bg-indigo-400 animate-ping"></span>
          <span class="text-xs font-medium text-indigo-300">Streaming (${res} @ ${fps}Hz)</span>
          <span class="text-[11px] font-mono text-emerald-400 bg-emerald-500/10 px-1.5 py-0.5 rounded border border-emerald-500/20">${latency}ms</span>
        </div>
      `;
    } else {
      pill.classList.add("hidden");
      pill.innerHTML = "";
    }
  }
}
