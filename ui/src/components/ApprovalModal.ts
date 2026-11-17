export interface IncomingRequest {
  peer_id: string;
  peer_name: string;
  pin: string;
  requested_at: number;
}

export class ApprovalModal {
  private container: HTMLElement;
  private currentRequest: IncomingRequest | null = null;
  private onRespondCallback: (peerId: string, approved: boolean) => Promise<void> | void;
  private onDismissCallback?: () => void;
  private isProcessing: boolean = false;

  constructor(
    containerId: string,
    onRespond: (peerId: string, approved: boolean) => Promise<void> | void,
    onDismiss?: () => void
  ) {
    const el = document.getElementById(containerId);
    if (!el) {
      throw new Error(`Container #${containerId} not found`);
    }
    this.container = el;
    this.onRespondCallback = onRespond;
    this.onDismissCallback = onDismiss;
    this.render();
  }

  public open(request: IncomingRequest): void {
    this.currentRequest = request;
    this.isProcessing = false;
    this.renderModal();
    this.container.classList.remove("hidden");
  }

  public close(): void {
    this.currentRequest = null;
    this.container.classList.add("hidden");
    this.container.innerHTML = "";
    this.onDismissCallback?.();
  }

  public isOpen(): boolean {
    return this.currentRequest !== null && !this.container.classList.contains("hidden");
  }

  public getCurrentPeerId(): string | null {
    return this.currentRequest ? this.currentRequest.peer_id : null;
  }

  public getCurrentRequest(): IncomingRequest | null {
    return this.currentRequest;
  }

  private render(): void {
    this.container.className =
      "fixed inset-0 bg-slate-950/80 backdrop-blur-sm z-50 flex items-center justify-center p-4 hidden";
  }

  private renderModal(): void {
    if (!this.currentRequest) return;

    this.container.innerHTML = `
      <div class="bg-slate-900 border border-slate-700/80 rounded-2xl p-6 max-w-sm w-full shadow-2xl relative animate-in fade-in zoom-in-95 duration-150">
        <!-- Header -->
        <div class="flex items-center justify-between mb-4">
          <div class="flex items-center space-x-2.5">
            <div class="w-9 h-9 rounded-xl bg-emerald-600/20 text-emerald-400 flex items-center justify-center font-bold text-base border border-emerald-500/20">
              🖥️
            </div>
            <div>
              <h3 class="text-sm font-semibold text-white">Incoming Display Request</h3>
              <p class="text-[11px] text-slate-400">Remote host wants to connect</p>
            </div>
          </div>
          <button id="approval-btn-close" class="text-slate-400 hover:text-slate-200 text-lg leading-none p-1 transition">
            &times;
          </button>
        </div>

        <!-- Peer Info Card -->
        <div class="p-3.5 bg-slate-950/70 rounded-xl border border-slate-800 space-y-1.5 mb-4">
          <div class="flex justify-between items-center text-xs">
            <span class="text-slate-400">Host Device:</span>
            <span class="font-semibold text-white">${this.currentRequest.peer_name}</span>
          </div>
          <div class="flex justify-between items-center text-xs">
            <span class="text-slate-400">Peer ID:</span>
            <span class="font-mono text-slate-300 text-[11px]">${this.currentRequest.peer_id}</span>
          </div>
        </div>

        <!-- PIN Verification Display -->
        <div class="mb-5 text-center">
          <div class="text-[11px] font-semibold text-slate-400 uppercase tracking-wider mb-1.5">
            Security Verification Code
          </div>
          <div class="p-4 bg-slate-950/90 rounded-xl border border-indigo-500/30">
            <div class="text-3xl font-mono font-bold tracking-[0.35em] text-indigo-400">
              ${this.currentRequest.pin}
            </div>
          </div>
          <p class="text-[11px] text-slate-400 mt-2">
            Verify that this 4-digit PIN matches the code shown on the sender's display before accepting.
          </p>
        </div>

        <div id="approval-error-banner" class="hidden mb-3 p-2 rounded-lg bg-rose-500/10 border border-rose-500/20 text-rose-400 text-xs text-center font-medium"></div>

        <!-- Action Buttons -->
        <div class="flex items-center space-x-2.5">
          <button
            id="approval-btn-reject"
            class="flex-1 px-4 py-2 text-xs font-semibold bg-rose-600/90 hover:bg-rose-500 text-white rounded-xl transition shadow-sm border border-rose-500/30"
          >
            Reject
          </button>
          <button
            id="approval-btn-accept"
            class="flex-1 px-4 py-2 text-xs font-semibold bg-emerald-600 hover:bg-emerald-500 text-white rounded-xl transition shadow-lg shadow-emerald-600/20 border border-emerald-500/30"
          >
            Accept & Connect
          </button>
        </div>
      </div>
    `;

    const closeBtn = document.getElementById("approval-btn-close");
    const rejectBtn = document.getElementById("approval-btn-reject");
    const acceptBtn = document.getElementById("approval-btn-accept");

    if (closeBtn) {
      closeBtn.onclick = () => this.handleResponse(false);
    }
    if (rejectBtn) {
      rejectBtn.onclick = () => this.handleResponse(false);
    }
    if (acceptBtn) {
      acceptBtn.onclick = () => this.handleResponse(true);
    }
  }

  private async handleResponse(approved: boolean): Promise<void> {
    if (!this.currentRequest || this.isProcessing) return;

    this.isProcessing = true;
    const acceptBtn = document.getElementById("approval-btn-accept") as HTMLButtonElement;
    const rejectBtn = document.getElementById("approval-btn-reject") as HTMLButtonElement;

    if (acceptBtn && rejectBtn) {
      acceptBtn.disabled = true;
      rejectBtn.disabled = true;
      if (approved) {
        acceptBtn.innerHTML = `
          <svg class="animate-spin -ml-1 mr-1.5 h-3.5 w-3.5 text-white inline" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
            <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
            <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8H4z"></path>
          </svg>
          Accepting...
        `;
      } else {
        rejectBtn.innerHTML = `Rejecting...`;
      }
    }

    try {
      await this.onRespondCallback(this.currentRequest.peer_id, approved);
      this.close();
    } catch (err: any) {
      const errEl = document.getElementById("approval-error-banner");
      if (errEl) {
        errEl.textContent = typeof err === "string" ? err : err.message || "Failed to respond";
        errEl.classList.remove("hidden");
      }
      this.isProcessing = false;
      if (acceptBtn && rejectBtn) {
        acceptBtn.disabled = false;
        rejectBtn.disabled = false;
        acceptBtn.textContent = "Accept & Connect";
        rejectBtn.textContent = "Reject";
      }
    }
  }
}
