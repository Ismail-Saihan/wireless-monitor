import { PeerDevice } from "./DeviceList";

export class ConnectIpModal {
  private container: HTMLElement;
  private onConnectCallback: (ip: string) => Promise<PeerDevice>;
  private onExtendCallback: (peer: PeerDevice) => void;
  private isConnecting: boolean = false;
  private currentDiscoveredPeer: PeerDevice | null = null;
  private localIp: string = "10.179.0.99";

  constructor(
    containerId: string,
    onConnect: (ip: string) => Promise<PeerDevice>,
    onExtend: (peer: PeerDevice) => void
  ) {
    const el = document.getElementById(containerId);
    if (!el) {
      throw new Error(`Container #${containerId} not found`);
    }
    this.container = el;
    this.onConnectCallback = onConnect;
    this.onExtendCallback = onExtend;
    this.render();
  }

  public setLocalIp(ip: string): void {
    this.localIp = ip;
  }

  public open(defaultIp: string = "10.179.0.245"): void {
    this.currentDiscoveredPeer = null;
    this.isConnecting = false;
    this.renderModal(defaultIp);
    this.container.classList.remove("hidden");

    setTimeout(() => {
      const input = document.getElementById("connect-ip-input") as HTMLInputElement | null;
      if (input) {
        input.focus();
        input.select();
      }
    }, 50);
  }

  public close(): void {
    this.container.classList.add("hidden");
    this.container.innerHTML = "";
    this.currentDiscoveredPeer = null;
  }

  public isOpen(): boolean {
    return !this.container.classList.contains("hidden");
  }

  private render(): void {
    this.container.className =
      "fixed inset-0 bg-black/75 backdrop-blur-md z-50 flex items-center justify-center p-4 hidden";
  }

  private renderModal(initialIp: string): void {
    this.container.innerHTML = `
      <div class="bg-slate-900/95 border border-indigo-500/30 rounded-2xl p-6 max-w-md w-full shadow-2xl relative animate-in fade-in zoom-in-95 duration-200 backdrop-blur-2xl">
        <!-- Header -->
        <div class="flex items-center justify-between mb-4">
          <div class="flex items-center space-x-3">
            <div class="w-10 h-10 rounded-xl bg-indigo-500/20 text-indigo-400 flex items-center justify-center font-bold text-base border border-indigo-500/30">
              <svg class="w-5 h-5 text-indigo-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 12a9 9 0 01-9 9m9-9a9 9 0 00-9-9m9 9H3m9 9a9 9 0 01-9-9m9 9c1.657 0 3-4.03 3-9s-1.343-9-3-9m0 18c-1.657 0-3-4.03-3-9s1.343-9 3-9m-9 9a9 9 0 019-9"/>
              </svg>
            </div>
            <div>
              <h3 class="text-sm font-semibold text-white">Connect by Direct IP</h3>
              <p class="text-[11px] text-slate-400">Bypasses mDNS multicast blocks across Wi-Fi routers</p>
            </div>
          </div>
          <button id="btn-close-ip-modal" class="text-slate-400 hover:text-white p-1 rounded-lg transition hover:bg-white/5">
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/>
            </svg>
          </button>
        </div>

        <!-- Input Section -->
        <div class="space-y-3 mb-4">
          <div>
            <label class="block text-xs font-medium text-slate-300 mb-1.5">
              Remote Device IP Address & Port
            </label>
            <div class="relative">
              <input
                type="text"
                id="connect-ip-input"
                value="${initialIp}"
                placeholder="e.g. 10.179.0.245 or 192.168.1.100:5000"
                class="w-full px-3.5 py-2.5 bg-black/50 border border-white/10 rounded-xl text-sm font-mono text-white placeholder-slate-500 focus:outline-none focus:border-indigo-500 focus:ring-1 focus:ring-indigo-500 transition"
              />
            </div>
          </div>

          <!-- Quick Suggestion Chips -->
          <div class="flex items-center space-x-2 text-[11px]">
            <span class="text-slate-400">Quick Test:</span>
            <button
              type="button"
              class="quick-ip-chip px-2.5 py-1 rounded-lg bg-indigo-500/15 hover:bg-indigo-500/25 border border-indigo-500/30 text-indigo-300 font-mono transition active:scale-95"
              data-ip="10.179.0.245"
            >
              10.179.0.245 (Target Laptop)
            </button>
            <button
              type="button"
              class="quick-ip-chip px-2.5 py-1 rounded-lg bg-white/5 hover:bg-white/10 border border-white/10 text-slate-300 font-mono transition active:scale-95"
              data-ip="127.0.0.1"
            >
              127.0.0.1 (Local)
            </button>
          </div>
        </div>

        <!-- Error Banner -->
        <div id="connect-ip-error" class="hidden mb-4 p-3 bg-rose-500/10 border border-rose-500/30 rounded-xl text-xs text-rose-300 font-medium"></div>

        <!-- Peer Result Card (appears after successful connect) -->
        <div id="connect-ip-result" class="hidden mb-4 p-3.5 bg-emerald-950/30 border border-emerald-500/40 rounded-xl space-y-2.5">
          <div class="flex items-center justify-between">
            <div class="flex items-center space-x-2.5">
              <span class="w-2.5 h-2.5 rounded-full bg-emerald-400 animate-pulse"></span>
              <div>
                <div id="result-peer-name" class="text-xs font-bold text-white">Device Found</div>
                <div id="result-peer-addr" class="text-[11px] font-mono text-emerald-400">10.179.0.245:5000</div>
              </div>
            </div>
            <span id="result-peer-role" class="text-[10px] px-2 py-0.5 rounded-full bg-emerald-500/20 text-emerald-300 border border-emerald-500/30 font-medium">
              Monitor Ready
            </span>
          </div>

          <!-- Badges -->
          <div id="result-peer-badges" class="flex flex-wrap gap-1 text-[10px] font-mono"></div>

          <!-- 1-Click Extend Screen Action -->
          <button
            type="button"
            id="btn-result-extend"
            class="w-full py-2 bg-gradient-to-r from-indigo-600 to-violet-600 hover:from-indigo-500 hover:to-violet-500 text-white rounded-lg font-semibold text-xs transition shadow-md shadow-indigo-950/50 flex items-center justify-center space-x-1.5 active:scale-98"
          >
            <span>Extend Screen Now</span>
            <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M14 5l7 7m0 0l-7 7m7-7H3"/>
            </svg>
          </button>
        </div>

        <!-- Action Buttons -->
        <div class="flex space-x-2.5">
          <button
            type="button"
            id="btn-scan-subnet"
            class="px-3.5 py-2 text-xs bg-white/5 hover:bg-white/10 text-slate-300 hover:text-white rounded-xl border border-white/10 font-medium transition active:scale-95 flex items-center justify-center space-x-1"
          >
            <svg class="w-3.5 h-3.5 text-indigo-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
            </svg>
            <span>Scan Subnet</span>
          </button>
          <button
            type="button"
            id="btn-connect-ip-submit"
            class="flex-1 py-2 text-xs bg-indigo-600 hover:bg-indigo-500 text-white rounded-xl font-medium transition shadow-md shadow-indigo-950/50 flex items-center justify-center space-x-1.5 active:scale-95"
          >
            <span>Connect to IP</span>
          </button>
        </div>
      </div>
    `;

    // Event listeners
    const closeBtn = document.getElementById("btn-close-ip-modal");
    if (closeBtn) closeBtn.onclick = () => this.close();

    const input = document.getElementById("connect-ip-input") as HTMLInputElement | null;
    if (input) {
      input.onkeydown = (e) => {
        if (e.key === "Enter") {
          e.preventDefault();
          this.handleConnect();
        } else if (e.key === "Escape") {
          this.close();
        }
      };
    }

    const submitBtn = document.getElementById("btn-connect-ip-submit");
    if (submitBtn) submitBtn.onclick = () => this.handleConnect();

    const scanBtn = document.getElementById("btn-scan-subnet");
    if (scanBtn) scanBtn.onclick = () => this.handleSubnetScan();

    const chips = this.container.querySelectorAll(".quick-ip-chip");
    chips.forEach((chip) => {
      chip.addEventListener("click", () => {
        const ip = chip.getAttribute("data-ip");
        if (ip && input) {
          input.value = ip;
          this.handleConnect();
        }
      });
    });

    // Close on clicking backdrop
    this.container.onclick = (e) => {
      if (e.target === this.container) {
        this.close();
      }
    };
  }

  private async handleConnect(): Promise<void> {
    if (this.isConnecting) return;
    const input = document.getElementById("connect-ip-input") as HTMLInputElement | null;
    const errorEl = document.getElementById("connect-ip-error");
    const resultCard = document.getElementById("connect-ip-result");
    const submitBtn = document.getElementById("btn-connect-ip-submit") as HTMLButtonElement | null;

    if (!input) return;
    const targetIp = input.value.trim();
    if (!targetIp) {
      if (errorEl) {
        errorEl.textContent = "Please enter an IP address";
        errorEl.classList.remove("hidden");
      }
      return;
    }

    if (errorEl) errorEl.classList.add("hidden");
    if (resultCard) resultCard.classList.add("hidden");

    this.isConnecting = true;
    if (submitBtn) {
      submitBtn.disabled = true;
      submitBtn.innerHTML = `
        <svg class="animate-spin -ml-1 mr-1.5 h-3.5 w-3.5 text-white inline" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
          <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
          <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8H4z"></path>
        </svg>
        Connecting...
      `;
    }

    try {
      const peer = await this.onConnectCallback(targetIp);
      this.currentDiscoveredPeer = peer;
      this.showPeerResult(peer);
    } catch (err: any) {
      console.error("Direct IP connection failed:", err);
      if (errorEl) {
        const msg = typeof err === "string" ? err : err.message || "Failed to reach device at that IP";
        errorEl.textContent = msg;
        errorEl.classList.remove("hidden");
      }
    } finally {
      this.isConnecting = false;
      if (submitBtn) {
        submitBtn.disabled = false;
        submitBtn.innerHTML = `<span>Connect to IP</span>`;
      }
    }
  }

  private showPeerResult(peer: PeerDevice): void {
    const resultCard = document.getElementById("connect-ip-result");
    const nameEl = document.getElementById("result-peer-name");
    const addrEl = document.getElementById("result-peer-addr");
    const roleEl = document.getElementById("result-peer-role");
    const badgesEl = document.getElementById("result-peer-badges");
    const extendBtn = document.getElementById("btn-result-extend");

    if (!resultCard) return;

    if (nameEl) nameEl.textContent = peer.name;
    if (addrEl) addrEl.textContent = `${peer.ip}:${peer.port}`;
    if (roleEl) roleEl.textContent = peer.mode === "monitor" ? "Monitor Ready" : "Host System";

    if (badgesEl) {
      const badges: string[] = [];
      if (peer.capabilities) {
        badges.push(...peer.capabilities);
      }
      if (peer.max_resolution && !badges.includes(peer.max_resolution)) {
        badges.push(peer.max_resolution);
      }
      if (peer.max_fps && !badges.some((b) => b.includes(`${peer.max_fps} FPS`))) {
        badges.push(`${peer.max_fps} FPS`);
      }

      badgesEl.innerHTML = badges
        .map(
          (b) =>
            `<span class="px-2 py-0.5 rounded bg-black/40 text-indigo-300 border border-white/5">${b}</span>`
        )
        .join("");
    }

    if (extendBtn) {
      extendBtn.onclick = () => {
        this.close();
        this.onExtendCallback(peer);
      };
    }

    resultCard.classList.remove("hidden");
  }

  private async handleSubnetScan(): Promise<void> {
    const input = document.getElementById("connect-ip-input") as HTMLInputElement | null;
    const scanBtn = document.getElementById("btn-scan-subnet") as HTMLButtonElement | null;
    if (!scanBtn) return;

    scanBtn.disabled = true;
    scanBtn.innerHTML = `
      <svg class="animate-spin -ml-1 mr-1 h-3 w-3 text-indigo-400 inline" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
        <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
        <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8H4z"></path>
      </svg>
      Scanning...
    `;

    // Try target candidate IPs: 10.179.0.245 first, then local subnet
    const candidates = ["10.179.0.245"];
    if (this.localIp && this.localIp.includes(".")) {
      const parts = this.localIp.split(".");
      if (parts.length === 4) {
        const prefix = `${parts[0]}.${parts[1]}.${parts[2]}`;
        const commonHosts = [245, 1, 2, 100, 101, 150];
        for (const host of commonHosts) {
          const cand = `${prefix}.${host}`;
          if (!candidates.includes(cand)) {
            candidates.push(cand);
          }
        }
      }
    }

    let foundPeer: PeerDevice | null = null;
    for (const cand of candidates) {
      try {
        const peer = await this.onConnectCallback(cand);
        if (peer) {
          foundPeer = peer;
          if (input) input.value = cand;
          this.showPeerResult(peer);
          break;
        }
      } catch {
        // Continue scanning next candidate
      }
    }

    scanBtn.disabled = false;
    scanBtn.innerHTML = `
      <svg class="w-3.5 h-3.5 text-indigo-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
      </svg>
      <span>Scan Subnet</span>
    `;

    if (!foundPeer) {
      const errorEl = document.getElementById("connect-ip-error");
      if (errorEl) {
        errorEl.textContent = "No active Wireless Monitor signaling listeners responded on the subnet.";
        errorEl.classList.remove("hidden");
      }
    }
  }
}
