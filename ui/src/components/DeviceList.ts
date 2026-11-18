export interface PeerDevice {
  id: string;
  name: string;
  ip: string;
  port: number;
  mode: "host" | "monitor" | string;
  capabilities?: string[];
  max_resolution?: string;
  max_fps?: number;
  last_seen?: number;
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
          <svg class="animate-spin -ml-1 mr-1.5 h-3.5 w-3.5 text-indigo-400 inline" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
            <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
            <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8H4z"></path>
          </svg>
          Scanning...
        `;
      } else {
        btn.innerHTML = `
          <svg class="w-3.5 h-3.5 mr-1.5 text-indigo-400 inline" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
          </svg>
          Scan LAN
        `;
      }
    }
  }

  private render(): void {
    this.container.innerHTML = `
      <div class="flex items-center justify-between mb-4">
        <div>
          <h2 class="text-sm font-semibold text-white flex items-center space-x-2">
            <span>Nearby Displays & Devices</span>
            <span class="text-[11px] px-2 py-0.5 bg-white/10 text-indigo-300 rounded-full font-mono font-medium">
              ${this.currentPeers.length}
            </span>
          </h2>
          <p class="text-[11px] text-slate-400 mt-0.5">Discovered via zero-config local mDNS</p>
        </div>
        <button id="btn-refresh-peers" class="text-xs px-3 py-1.5 bg-white/5 hover:bg-white/10 text-slate-200 hover:text-white font-medium rounded-xl border border-white/10 transition-all flex items-center active:scale-95 shadow-sm">
          <svg class="w-3.5 h-3.5 mr-1.5 text-indigo-400 inline" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
          </svg>
          Scan LAN
        </button>
      </div>

      <div id="peers-inner-list" class="space-y-3"></div>
    `;

    const refreshBtn = document.getElementById("btn-refresh-peers");
    if (refreshBtn) {
      refreshBtn.onclick = () => this.onRefreshClick();
    }

    const listEl = document.getElementById("peers-inner-list");
    if (!listEl) return;

    if (this.currentPeers.length === 0) {
      listEl.innerHTML = `
        <div class="text-center py-16 px-6 border border-dashed border-white/10 rounded-2xl bg-black/20 flex flex-col items-center justify-center relative overflow-hidden">
          <!-- Animated Radar Scanner -->
          <div class="relative w-20 h-20 mb-4 flex items-center justify-center">
            <div class="absolute inset-0 rounded-full bg-indigo-500/20 animate-radar"></div>
            <div class="absolute inset-2 rounded-full bg-indigo-500/30 animate-ping"></div>
            <div class="relative w-12 h-12 rounded-full bg-gradient-to-tr from-indigo-600/40 to-violet-500/40 border border-indigo-500/40 flex items-center justify-center text-indigo-400 shadow-lg shadow-indigo-500/20">
              <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8.111 16.404a5.5 5.5 0 017.778 0M12 20h.01m-7.08-7.071a10 10 0 0114.142 0M1.393 9.393a15 15 0 0121.214 0" />
              </svg>
            </div>
          </div>
          <h3 class="text-sm font-semibold text-slate-200">Searching for Nearby Devices</h3>
          <p class="text-xs text-slate-400 mt-1 max-w-sm">
            Launch Wireless Monitor on another laptop, PC, or tablet connected to the same Wi-Fi network.
          </p>
          <div class="mt-4 flex items-center space-x-2 text-[11px] text-slate-500 font-mono">
            <span>mDNS Service:</span>
            <span class="text-indigo-400 bg-white/5 px-2 py-0.5 rounded border border-white/5">_wireless-monitor._tcp</span>
          </div>
        </div>
      `;
      return;
    }

    this.currentPeers.forEach((peer) => {
      const card = document.createElement("div");
      card.className =
        "p-4 rounded-xl bg-slate-900/70 hover:bg-slate-900/95 border border-white/[0.08] hover:border-indigo-500/40 transition-all duration-200 shadow-md space-y-3";

      const isMonitor = peer.mode === "monitor";
      const badgeClass = isMonitor
        ? "bg-indigo-500/15 text-indigo-300 border-indigo-500/30"
        : "bg-emerald-500/15 text-emerald-300 border-emerald-500/30";
      const modeLabel = isMonitor ? "Monitor Ready" : "Host System";

      // Collect capability tags
      const badges: string[] = [];
      if (peer.capabilities && peer.capabilities.length > 0) {
        for (const cap of peer.capabilities) {
          if (!badges.includes(cap)) {
            badges.push(cap);
          }
        }
      }
      if (peer.max_resolution && !badges.some((b) => b.toLowerCase().includes(peer.max_resolution!.toLowerCase()))) {
        badges.push(peer.max_resolution);
      }
      if (peer.max_fps && !badges.some((b) => b.includes(`${peer.max_fps} FPS`))) {
        badges.push(`${peer.max_fps} FPS`);
      }

      const badgesHtml = badges.length > 0
        ? `<div class="flex flex-wrap gap-1.5 pt-1">
            ${badges
              .map(
                (badge) =>
                  `<span class="text-[10px] px-2 py-0.5 rounded-md bg-black/40 text-indigo-300 border border-white/5 font-mono">${badge}</span>`
              )
              .join("")}
          </div>`
        : "";

      card.innerHTML = `
        <div class="flex items-center justify-between gap-3">
          <div class="flex items-center space-x-3.5">
            <div class="w-10 h-10 rounded-xl ${isMonitor ? "bg-indigo-500/15 border-indigo-500/30 text-indigo-400" : "bg-emerald-500/15 border-emerald-500/30 text-emerald-400"} border flex items-center justify-center shrink-0">
              ${isMonitor
                ? `<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9.75 17L9 20l-1 1h8l-1-1-.75-3M3 13h18M5 17h14a2 2 0 002-2V5a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z"/></svg>`
                : `<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9.75 17L9 20l-1 1h8l-1-1-.75-3M3 13h18M5 17h14a2 2 0 002-2V5a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z"/></svg>`}
            </div>
            <div>
              <div class="flex items-center space-x-2">
                <span class="text-xs font-semibold text-white tracking-wide">${peer.name}</span>
                <span class="text-[10px] px-2 py-0.5 rounded-full border font-medium ${badgeClass}">
                  ${modeLabel}
                </span>
              </div>
              <div class="text-[11px] text-slate-400 font-mono mt-0.5">
                ${peer.ip}:${peer.port}
              </div>
            </div>
          </div>
          <div class="flex items-center space-x-2 shrink-0">
            <button data-action="extend" class="px-3.5 py-1.5 text-xs bg-gradient-to-r from-indigo-600 to-violet-600 hover:from-indigo-500 hover:to-violet-500 text-white rounded-lg font-medium transition shadow-md shadow-indigo-950/50 active:scale-95 flex items-center space-x-1">
              <span>Extend Screen</span>
              <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M14 5l7 7m0 0l-7 7m7-7H3"/>
              </svg>
            </button>
            <button data-action="monitor" class="px-3 py-1.5 text-xs bg-white/5 hover:bg-white/10 text-slate-300 hover:text-white rounded-lg border border-white/10 font-medium transition active:scale-95">
              Use as Monitor
            </button>
          </div>
        </div>
        ${badgesHtml}
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
