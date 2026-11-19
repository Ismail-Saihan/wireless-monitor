import { invoke } from "@tauri-apps/api/core";
import { PeerDevice } from "./DeviceList";

export class PairingModal {
  private container: HTMLElement;
  private currentPeer: PeerDevice | null = null;
  private onSubmitCallback: (peerId: string, pin: string) => Promise<boolean>;
  private onCancelCallback: () => void;
  private attemptsRemaining: number = 3;
  private isSubmitting: boolean = false;
  private approvalPollTimer: number | null = null;

  constructor(
    containerId: string,
    onSubmit: (peerId: string, pin: string) => Promise<boolean>,
    onCancel: () => void
  ) {
    const el = document.getElementById(containerId);
    if (!el) {
      throw new Error(`Container #${containerId} not found`);
    }
    this.container = el;
    this.onSubmitCallback = onSubmit;
    this.onCancelCallback = onCancel;
    this.render();
  }

  public open(peer: PeerDevice, pinHint?: string | null, isPendingApproval: boolean = false): void {
    this.clearApprovalPollTimer();
    this.currentPeer = peer;
    this.attemptsRemaining = 3;
    this.isSubmitting = false;
    this.renderModal(pinHint, isPendingApproval);
    this.container.classList.remove("hidden");

    if (isPendingApproval && peer.ip) {
      this.startApprovalPolling(peer, pinHint);
    }

    // Focus input
    setTimeout(() => {
      const input = document.getElementById("pin-input-field") as HTMLInputElement;
      if (input) {
        if (pinHint) {
          input.value = pinHint;
        } else {
          input.value = "";
        }
        input.focus();
        input.select();
        this.updateSubmitButtonState();
      }
    }, 50);
  }

  public close(): void {
    this.clearApprovalPollTimer();
    this.currentPeer = null;
    this.container.classList.add("hidden");
    this.container.innerHTML = "";
  }

  private clearApprovalPollTimer(): void {
    if (this.approvalPollTimer !== null) {
      clearInterval(this.approvalPollTimer);
      this.approvalPollTimer = null;
    }
  }

  private startApprovalPolling(peer: PeerDevice, pinHint?: string | null): void {
    this.clearApprovalPollTimer();
    const peerIp = peer.ip;
    const peerPort = peer.port || 5000;

    this.approvalPollTimer = window.setInterval(async () => {
      try {
        const resp = await invoke<{ status: string; pin?: string | null }>(
          "query_remote_connection_status",
          {
            ip: peerIp,
            port: peerPort,
            peerId: peer.id,
          }
        );

        if (resp.status === "approved" || resp.status === "authenticated") {
          this.clearApprovalPollTimer();
          const waitingCard = document.getElementById("approval-waiting-card");
          if (waitingCard) {
            waitingCard.className =
              "mb-4 p-3.5 bg-emerald-500/10 border border-emerald-500/30 rounded-xl text-center flex items-center space-x-3";
            waitingCard.innerHTML = `
              <span class="w-3 h-3 rounded-full bg-emerald-400 animate-ping"></span>
              <div class="text-left">
                <div class="text-xs font-semibold text-emerald-300">Connection Approved!</div>
                <div class="text-[11px] text-slate-300">Starting display stream...</div>
              </div>
            `;
          }

          const targetPin = resp.pin || pinHint;
          if (targetPin) {
            this.isSubmitting = true;
            this.updateSubmitButtonState();
            try {
              const ok = await this.onSubmitCallback(peer.id, targetPin);
              if (ok) {
                this.close();
              }
            } finally {
              this.isSubmitting = false;
              this.updateSubmitButtonState();
            }
          }
        } else if (resp.status === "rejected") {
          this.clearApprovalPollTimer();
          this.setError(`Connection request was declined by ${peer.name}`);
        }
      } catch (e) {
        // Ignore transient poll error
      }
    }, 800);
  }

  public setError(errorMsg: string, attemptsRemaining?: number): void {
    this.isSubmitting = false;
    if (attemptsRemaining !== undefined) {
      this.attemptsRemaining = attemptsRemaining;
    }
    const errorEl = document.getElementById("modal-error-banner");
    const attemptsEl = document.getElementById("modal-attempts-counter");
    if (errorEl) {
      errorEl.textContent = errorMsg;
      errorEl.classList.remove("hidden");
    }
    if (attemptsEl) {
      attemptsEl.textContent = `${this.attemptsRemaining} attempt${this.attemptsRemaining === 1 ? "" : "s"} remaining`;
      if (this.attemptsRemaining <= 1) {
        attemptsEl.className = "text-xs font-medium text-rose-400";
      }
    }
    this.updateSubmitButtonState();
  }

  private render(): void {
    this.container.className =
      "fixed inset-0 bg-black/75 backdrop-blur-md z-50 flex items-center justify-center p-4 hidden";
  }

  private renderModal(pinHint?: string | null, isPendingApproval: boolean = false): void {
    if (!this.currentPeer) return;

    const waitingHtml = isPendingApproval
      ? `
        <div id="approval-waiting-card" class="mb-4 p-3.5 bg-amber-500/10 border border-amber-500/30 rounded-xl text-center flex items-center space-x-3">
          <svg class="animate-spin h-5 w-5 text-amber-400 shrink-0" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
            <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
            <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8H4z"></path>
          </svg>
          <div class="text-left">
            <div class="text-xs font-semibold text-amber-300">Awaiting Acceptance on Receiver</div>
            <div class="text-[11px] text-slate-300">Please click <strong class="text-white">Accept Display</strong> on ${this.currentPeer.name}</div>
          </div>
        </div>
      `
      : "";

    const pinHintHtml = pinHint
      ? `
        <div class="mb-4 p-4 bg-indigo-500/10 border border-indigo-500/30 rounded-xl text-center">
          <div class="text-[11px] text-indigo-300 font-medium uppercase tracking-wider mb-2">Display Pairing PIN</div>
          <div class="text-3xl font-mono font-bold tracking-[0.35em] text-indigo-400 select-all mb-1.5">
            ${pinHint}
          </div>
          <p class="text-[11px] text-slate-400">
            Security code for <strong class="text-white">${this.currentPeer.name}</strong>.
          </p>
        </div>
      `
      : "";

    this.container.innerHTML = `
      <div class="bg-slate-900/95 border border-white/10 rounded-2xl p-6 max-w-sm w-full shadow-2xl relative animate-in fade-in zoom-in-95 duration-200 backdrop-blur-2xl">
        <div class="flex items-center justify-between mb-4">
          <div class="flex items-center space-x-3">
            <div class="w-9 h-9 rounded-xl bg-indigo-500/20 text-indigo-400 flex items-center justify-center font-bold text-base border border-indigo-500/30">
              <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z"/>
              </svg>
            </div>
            <div>
              <h3 class="text-sm font-semibold text-white">Pairing Security</h3>
              <p class="text-[11px] text-slate-400">Connecting to ${this.currentPeer.name}</p>
            </div>
          </div>
          <button id="modal-btn-close" class="text-slate-400 hover:text-white p-1 rounded-lg transition hover:bg-white/5">
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/>
            </svg>
          </button>
        </div>

        ${pinHintHtml}

        <div id="modal-error-banner" class="hidden mb-3 p-2.5 rounded-xl bg-rose-500/10 border border-rose-500/25 text-rose-300 text-xs"></div>

        <form id="pairing-pin-form" class="space-y-4">
          <div>
            <label class="block text-xs font-medium text-slate-300 mb-1.5 text-center">
              Enter 4-Digit Security PIN
            </label>
            <input
              type="text"
              id="pin-input-field"
              maxlength="4"
              inputmode="numeric"
              pattern="[0-9]{4}"
              placeholder="••••"
              autocomplete="off"
              class="w-full text-center text-2xl tracking-[0.4em] font-mono py-2.5 px-4 rounded-xl bg-black/40 border border-white/10 text-white placeholder-slate-600 focus:outline-none focus:border-indigo-500 focus:ring-2 focus:ring-indigo-500/20 transition"
              required
            />
            <div class="flex justify-between items-center mt-2 px-1">
              <span id="modal-attempts-counter" class="text-[11px] text-slate-400">
                ${this.attemptsRemaining} attempts remaining
              </span>
              <span class="text-[11px] text-slate-500">Auto-expires in 5m</span>
            </div>
          </div>

          <div class="flex space-x-2.5 pt-2">
            <button
              type="button"
              id="modal-btn-cancel"
              class="flex-1 py-2 text-xs bg-white/5 hover:bg-white/10 text-slate-300 hover:text-white rounded-xl border border-white/10 font-medium transition"
            >
              Cancel
            </button>
            <button
              type="submit"
              id="modal-btn-submit"
              class="flex-1 py-2 text-xs bg-gradient-to-r from-indigo-600 to-violet-600 hover:from-indigo-500 hover:to-violet-500 text-white rounded-xl font-medium transition shadow-md shadow-indigo-950/50 flex items-center justify-center space-x-1.5"
            >
              <span>Connect</span>
            </button>
          </div>
        </form>
      </div>
    `;

    // Event listeners
    const form = document.getElementById("pairing-pin-form");
    const input = document.getElementById("pin-input-field") as HTMLInputElement;
    const cancelBtn = document.getElementById("modal-btn-cancel");
    const closeBtn = document.getElementById("modal-btn-close");

    if (closeBtn) {
      closeBtn.onclick = () => {
        this.close();
        this.onCancelCallback();
      };
    }

    if (cancelBtn) {
      cancelBtn.onclick = () => {
        this.close();
        this.onCancelCallback();
      };
    }

    if (input) {
      input.oninput = () => {
        input.value = input.value.replace(/[^0-9]/g, "");
        this.updateSubmitButtonState();
      };
    }

    if (form) {
      form.onsubmit = async (e) => {
        e.preventDefault();
        if (!this.currentPeer || this.isSubmitting) return;

        const pinVal = input.value.trim();
        if (pinVal.length !== 4) {
          this.setError("PIN must be exactly 4 digits");
          return;
        }

        this.isSubmitting = true;
        this.updateSubmitButtonState();

        try {
          const success = await this.onSubmitCallback(this.currentPeer.id, pinVal);
          if (success) {
            this.close();
          } else {
            this.attemptsRemaining = Math.max(0, this.attemptsRemaining - 1);
            this.setError("Invalid PIN, verification failed", this.attemptsRemaining);
          }
        } catch (err: any) {
          this.attemptsRemaining = Math.max(0, this.attemptsRemaining - 1);
          this.setError(`Pairing error: ${err?.message || err}`, this.attemptsRemaining);
        } finally {
          this.isSubmitting = false;
          this.updateSubmitButtonState();
        }
      };
    }
  }

  private updateSubmitButtonState(): void {
    const btn = document.getElementById("modal-btn-submit") as HTMLButtonElement | null;
    const input = document.getElementById("pin-input-field") as HTMLInputElement | null;
    if (!btn || !input) return;

    if (this.isSubmitting) {
      btn.disabled = true;
      btn.innerHTML = `
        <svg class="animate-spin -ml-1 mr-1.5 h-3 w-3 text-white inline" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
          <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
          <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8H4z"></path>
        </svg>
        Pairing...
      `;
    } else {
      btn.disabled = input.value.length !== 4;
      btn.textContent = "Connect";
    }
  }
}
