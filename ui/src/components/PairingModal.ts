import { PeerDevice } from "./DeviceList";

export class PairingModal {
  private container: HTMLElement;
  private currentPeer: PeerDevice | null = null;
  private onSubmitCallback: (peerId: string, pin: string) => Promise<boolean>;
  private onCancelCallback: () => void;
  private attemptsRemaining: number = 3;
  private isSubmitting: boolean = false;

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

  public open(peer: PeerDevice): void {
    this.currentPeer = peer;
    this.attemptsRemaining = 3;
    this.isSubmitting = false;
    this.renderModal();
    this.container.classList.remove("hidden");

    // Focus input
    setTimeout(() => {
      const input = document.getElementById("pin-input-field") as HTMLInputElement;
      if (input) {
        input.value = "";
        input.focus();
      }
    }, 50);
  }

  public close(): void {
    this.currentPeer = null;
    this.container.classList.add("hidden");
    this.container.innerHTML = "";
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
      "fixed inset-0 bg-slate-950/80 backdrop-blur-sm z-50 flex items-center justify-center p-4 hidden";
  }

  private renderModal(): void {
    if (!this.currentPeer) return;

    this.container.innerHTML = `
      <div class="bg-slate-900 border border-slate-700/80 rounded-2xl p-6 max-w-sm w-full shadow-2xl relative animate-in fade-in zoom-in-95 duration-150">
        <div class="flex items-center justify-between mb-4">
          <div class="flex items-center space-x-2">
            <div class="w-8 h-8 rounded-lg bg-indigo-600/20 text-indigo-400 flex items-center justify-center font-bold text-sm">
              🔒
            </div>
            <div>
              <h3 class="text-sm font-semibold text-white">Pairing Verification</h3>
              <p class="text-[11px] text-slate-400">Connect to ${this.currentPeer.name}</p>
            </div>
          </div>
          <button id="modal-btn-close" class="text-slate-400 hover:text-slate-200 text-lg leading-none p-1">
            &times;
          </button>
        </div>

        <div id="modal-error-banner" class="hidden mb-3 p-2.5 rounded-lg bg-rose-500/10 border border-rose-500/20 text-rose-400 text-xs text-center font-medium"></div>

        <div class="mb-4">
          <label class="block text-xs font-medium text-slate-300 mb-1.5 text-center">
            Enter 4-Digit Security PIN
          </label>
          <div class="flex justify-center">
            <input
              id="pin-input-field"
              type="text"
              inputmode="numeric"
              pattern="[0-9]*"
              maxlength="4"
              placeholder="••••"
              class="w-44 text-center tracking-[0.5em] font-mono text-2xl py-2 px-3 bg-slate-950 border border-slate-700 rounded-xl text-white placeholder-slate-600 focus:outline-none focus:ring-2 focus:ring-indigo-500 focus:border-transparent transition"
            />
          </div>
          <p id="modal-attempts-counter" class="text-xs text-slate-400 text-center mt-2">
            ${this.attemptsRemaining} attempts remaining
          </p>
        </div>

        <div class="flex items-center space-x-2">
          <button
            id="modal-btn-cancel"
            class="flex-1 px-3 py-2 text-xs bg-slate-800 hover:bg-slate-750 text-slate-300 rounded-xl font-medium border border-slate-700/60 transition"
          >
            Cancel
          </button>
          <button
            id="modal-btn-submit"
            class="flex-1 px-3 py-2 text-xs bg-indigo-600 hover:bg-indigo-500 disabled:opacity-50 disabled:cursor-not-allowed text-white rounded-xl font-medium transition shadow-lg shadow-indigo-600/30"
          >
            Pair & Connect
          </button>
        </div>
      </div>
    `;

    // Hook events
    const closeBtn = document.getElementById("modal-btn-close");
    const cancelBtn = document.getElementById("modal-btn-cancel");
    const submitBtn = document.getElementById("modal-btn-submit");
    const input = document.getElementById("pin-input-field") as HTMLInputElement;

    const handleCancel = () => {
      this.close();
      this.onCancelCallback();
    };

    if (closeBtn) closeBtn.onclick = handleCancel;
    if (cancelBtn) cancelBtn.onclick = handleCancel;

    if (submitBtn) {
      submitBtn.onclick = () => this.handleFormSubmit();
    }

    if (input) {
      input.onkeydown = (e) => {
        if (e.key === "Enter") {
          e.preventDefault();
          this.handleFormSubmit();
        } else if (e.key === "Escape") {
          e.preventDefault();
          handleCancel();
        }
      };

      input.oninput = () => {
        // Only allow numbers
        input.value = input.value.replace(/[^0-9]/g, "");
      };
    }
  }

  private async handleFormSubmit(): Promise<void> {
    if (!this.currentPeer || this.isSubmitting) return;

    const input = document.getElementById("pin-input-field") as HTMLInputElement;
    const pin = input ? input.value.trim() : "";

    if (pin.length !== 4) {
      this.setError("PIN must be exactly 4 numeric digits");
      return;
    }

    this.isSubmitting = true;
    this.updateSubmitButtonState();

    try {
      const ok = await this.onSubmitCallback(this.currentPeer.id, pin);
      if (ok) {
        this.close();
      }
    } catch (err: any) {
      const msg = err ? (err.message || String(err)) : "Failed to verify PIN";
      this.attemptsRemaining = Math.max(0, this.attemptsRemaining - 1);
      this.setError(msg, this.attemptsRemaining);
    }
  }

  private updateSubmitButtonState(): void {
    const submitBtn = document.getElementById("modal-btn-submit") as HTMLButtonElement;
    if (!submitBtn) return;

    if (this.isSubmitting) {
      submitBtn.disabled = true;
      submitBtn.innerHTML = `
        <svg class="animate-spin -ml-1 mr-1.5 h-3.5 w-3.5 text-white inline" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
          <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
          <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8H4z"></path>
        </svg>
        Pairing...
      `;
    } else {
      submitBtn.disabled = this.attemptsRemaining <= 0;
      submitBtn.textContent = "Pair & Connect";
    }
  }
}
