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
      "fixed inset-0 bg-black/75 backdrop-blur-md z-50 flex items-center justify-center p-4 hidden";
  }

  private renderModal(): void {
    if (!this.currentRequest) return;

    this.container.innerHTML = `
      <div class="bg-slate-900/95 border border-emerald-500/30 rounded-2xl p-6 max-w-sm w-full shadow-2xl relative animate-in fade-in zoom-in-95 duration-200 backdrop-blur-2xl">
        <!-- Header -->
        <div class="flex items-center justify-between mb-4">
          <div class="flex items-center space-x-3">
            <div class="w-10 h-10 rounded-xl bg-emerald-500/20 text-emerald-400 flex items-center justify-center font-bold text-base border border-emerald-500/30">
              <svg class="w-5 h-5 text-emerald-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9.75 17L9 20l-1 1h8l-1-1-.75-3M3 13h18M5 17h14a2 2 0 002-2V5a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z"/>
              </svg>
            </div>
            <div>
              <h3 class="text-sm font-semibold text-white">Incoming Display Request</h3>
              <p class="text-[11px] text-slate-400">Remote host wants to project to this PC</p>
            </div>
          </div>
          <button id="approval-btn-close" class="text-slate-400 hover:text-white p-1 rounded-lg transition hover:bg-white/5">
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/>
            </svg>
          </button>
        </div>

        <!-- Peer Info Card -->
        <div class="p-3.5 bg-black/40 rounded-xl border border-white/5 space-y-1.5 mb-4">
          <div class="flex justify-between items-center text-xs">
            <span class="text-slate-400">Host Computer:</span>
            <span class="font-semibold text-white">${this.currentRequest.peer_name}</span>
          </div>
          <div class="flex justify-between items-center text-xs">
            <span class="text-slate-400">Peer ID:</span>
            <span class="font-mono text-slate-300 text-[11px]">${this.currentRequest.peer_id}</span>
          </div>
        </div>

        <!-- PIN Verification Display -->
        <div class="mb-5 text-center">
          <div class="text-[10px] font-semibold text-slate-400 uppercase tracking-wider mb-1.5">
            Security Verification Code
          </div>
          <div class="p-4 bg-black/60 rounded-xl border border-indigo-500/30">
            <div class="text-3xl font-mono font-bold tracking-[0.35em] text-indigo-400">
              ${this.currentRequest.pin}
            </div>
          </div>
          <p class="text-[11px] text-slate-500 mt-2">
            Confirm that this code matches the code shown on the host device.
          </p>
        </div>

        <!-- Action Buttons -->
        <div class="flex space-x-2.5">
          <button
            type="button"
            id="approval-btn-reject"
            class="flex-1 py-2 text-xs bg-white/5 hover:bg-rose-500/10 text-slate-300 hover:text-rose-400 rounded-xl border border-white/10 hover:border-rose-500/30 font-medium transition"
          >
            Decline
          </button>
          <button
            type="button"
            id="approval-btn-approve"
            class="flex-1 py-2 text-xs bg-gradient-to-r from-emerald-600 to-teal-600 hover:from-emerald-500 hover:to-teal-500 text-white rounded-xl font-medium transition shadow-md shadow-emerald-950/50 flex items-center justify-center space-x-1.5"
          >
            <span>Accept Display</span>
          </button>
        </div>
      </div>
    `;

    // Event listeners
    const approveBtn = document.getElementById("approval-btn-approve") as HTMLButtonElement | null;
    const rejectBtn = document.getElementById("approval-btn-reject") as HTMLButtonElement | null;
    const closeBtn = document.getElementById("approval-btn-close");

    if (closeBtn) {
      closeBtn.onclick = () => this.handleResponse(false);
    }
    if (rejectBtn) {
      rejectBtn.onclick = () => this.handleResponse(false);
    }
    if (approveBtn) {
      approveBtn.onclick = () => this.handleResponse(true);
    }
  }

  private async handleResponse(approved: boolean): Promise<void> {
    if (!this.currentRequest || this.isProcessing) return;
    this.isProcessing = true;

    const approveBtn = document.getElementById("approval-btn-approve") as HTMLButtonElement | null;
    const rejectBtn = document.getElementById("approval-btn-reject") as HTMLButtonElement | null;

    if (approveBtn) {
      approveBtn.disabled = true;
      if (approved) {
        approveBtn.innerHTML = `
          <svg class="animate-spin -ml-1 mr-1.5 h-3 w-3 text-white inline" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
            <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
            <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8H4z"></path>
          </svg>
          Accepting...
        `;
      }
    }
    if (rejectBtn) {
      rejectBtn.disabled = true;
    }

    try {
      await this.onRespondCallback(this.currentRequest.peer_id, approved);
      this.close();
    } catch (err) {
      console.error("Approval action failed:", err);
      this.isProcessing = false;
      if (approveBtn) approveBtn.disabled = false;
      if (rejectBtn) rejectBtn.disabled = false;
    }
  }
}
