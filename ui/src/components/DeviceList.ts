export interface PeerDevice {
  id: string;
  name: string;
  ip: string;
  port: number;
  mode: "host" | "monitor" | string;
}

export class DeviceList {
  private container: HTMLElement;
  private onExtendClick: (peer: PeerDevice) => void;
  private onUseAsMonitorClick: (peer: PeerDevice) => void;
  private onRefreshClick: () => void;
  private currentPeers: PeerDevice[] = [];
  private isLoading: boolean = false;

  constructor(
    containerId: string,
    onExtendClick: (peer: PeerDevice) => void,
    onUseAsMonitorClick: (peer: PeerDevice) => void,
    onRefreshClick: () => void
  ) {
    const el = document.getElementById(containerId);
    if (!el) {
      throw new Error(`Container #${containerId} not found`);
    }
    this.container = el;
    this.onExtendClick = onExtendClick;
    this.onUseAsMonitorClick = onUseAsMonitorClick;
    this.onRefreshClick = onRefreshClick;
    this.render();
  }

  public setPeers(peers: PeerDevice[]): void {
    this.currentPeers = peers;
    this.render();
  }

  public setLoading(loading: boolean): void {
    this.isLoading = loading;
    const btn = document.getElementById("btn-refresh-peers");
    if (btn) {
      if (loading) {
        btn.innerHTML = `
          <svg class="animate-spin -ml-1 mr-1.5 h-3 w-3 text-indigo-400 inline" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
            <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
            <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8H4z"></path>
          </svg>
          Scanning...
        `;
      } else {
        btn.textContent = "Scan LAN";
      }
    }
  }

  private render(): void {
    this.container.innerHTML = `
      <div class="flex items-center justify-between mb-3">
        <div>
          <h2 class="text-sm font-semibold text-emerald-400 flex items-center space-x-2">
            <span>Nearby Devices</span>
            <span class="text-[11px] px-2 py-0.2 bg-slate-800 text-slate-400 rounded-full font-mono font-normal">
              ${this.currentPeers.length}
            </span>
          </h2>
          <p class="text-xs text-slate-400 mt-0.5">mDNS devices discovered on local subnet</p>
        </div>
        <button id="btn-refresh-peers" class="text-xs px-2.5 py-1 bg-slate-850 hover:bg-slate-700/80 text-indigo-400 hover:text-indigo-300 font-medium rounded-lg border border-slate-700/50 transition">
          ${this.isLoading ? "Scanning..." : "Scan LAN"}
        </button>
      </div>

      <div id="peers-inner-list" class="space-y-2.5 mt-3"></div>
    `;

    const refreshBtn = document.getElementById("btn-refresh-peers");
    if (refreshBtn) {
      refreshBtn.onclick = () => this.onRefreshClick();
    }

    const listEl = document.getElementById("peers-inner-list");
    if (!listEl) return;

    if (this.currentPeers.length === 0) {
      listEl.innerHTML = `
        <div class="text-center py-10 px-4 border border-dashed border-slate-800 rounded-xl bg-slate-900/30">
          <div class="w-8 h-8 mx-auto mb-2 rounded-full bg-slate-800 flex items-center justify-center text-slate-500">
            <svg class="w-4 h-4 animate-pulse" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8.111 16.404a5.5 5.5 0 017.778 0M12 20h.01m-7.08-7.071a10 10 0 0114.142 0M1.393 9.393a15 15 0 0121.214 0" />
            </svg>
          </div>
          <p class="text-xs text-slate-400 font-medium">Scanning for devices over mDNS...</p>
          <p class="text-[11px] text-slate-500 mt-1">Make sure devices are connected to the same Wi-Fi / LAN network.</p>
        </div>
      `;
      return;
    }

    this.currentPeers.forEach((peer) => {
      const card = document.createElement("div");
      card.className =
        "flex items-center justify-between p-3.5 bg-slate-900/60 hover:bg-slate-900/90 rounded-xl border border-slate-700/50 transition shadow-sm";

      const isMonitor = peer.mode === "monitor";
      const badgeClass = isMonitor
        ? "bg-indigo-500/10 text-indigo-400 border-indigo-500/20"
        : "bg-emerald-500/10 text-emerald-400 border-emerald-500/20";
      const modeLabel = isMonitor ? "Monitor" : "Host";

      card.innerHTML = `
        <div class="flex items-center space-x-3">
          <div class="w-8 h-8 rounded-lg ${isMonitor ? "bg-indigo-600/20 text-indigo-400" : "bg-emerald-600/20 text-emerald-400"} flex items-center justify-center text-sm font-semibold border border-slate-700/60">
            ${isMonitor ? "🖥️" : "💻"}
          </div>
          <div>
            <div class="flex items-center space-x-2">
              <span class="text-xs font-semibold text-white tracking-wide">${peer.name}</span>
              <span class="text-[10px] px-1.5 py-0.2 rounded-full border ${badgeClass}">
                ${modeLabel}
              </span>
            </div>
            <div class="text-[11px] text-slate-400 font-mono mt-0.5">
              ${peer.ip}:${peer.port}
            </div>
          </div>
        </div>
        <div class="flex items-center space-x-2">
          <button data-action="extend" class="px-3 py-1.5 text-xs bg-indigo-600 hover:bg-indigo-500 text-white rounded-lg font-medium transition shadow-sm">
            Extend Display
          </button>
          <button data-action="monitor" class="px-2.5 py-1.5 text-xs bg-slate-800 hover:bg-slate-750 text-slate-300 hover:text-white rounded-lg border border-slate-700/60 font-medium transition">
            Use as Monitor
          </button>
        </div>
      `;

      const extendBtn = card.querySelector('[data-action="extend"]');
      if (extendBtn) {
        extendBtn.addEventListener("click", () => this.onExtendClick(peer));
      }

      const monitorBtn = card.querySelector('[data-action="monitor"]');
      if (monitorBtn) {
        monitorBtn.addEventListener("click", () => this.onUseAsMonitorClick(peer));
      }

      listEl.appendChild(card);
    });
  }
}
