import { invoke } from "@tauri-apps/api/core";

export interface StreamSettings {
  resolution_mode: "auto" | "1080p" | "1440p" | "4k" | "custom" | string;
  custom_width?: number | null;
  custom_height?: number | null;
  framerate: number;
  bitrate_kbps: number;
}

export const DEFAULT_STREAM_SETTINGS: StreamSettings = {
  resolution_mode: "auto",
  custom_width: null,
  custom_height: null,
  framerate: 60,
  bitrate_kbps: 20000,
};

const STORAGE_KEY = "wm_stream_settings";

export class SettingsDrawer {
  private container: HTMLElement;
  private currentSettings: StreamSettings = { ...DEFAULT_STREAM_SETTINGS };
  private onSaveCallback?: (settings: StreamSettings) => Promise<void> | void;
  private onCloseCallback?: () => void;
  private isSaving: boolean = false;

  constructor(
    containerId: string,
    onSave?: (settings: StreamSettings) => Promise<void> | void,
    onClose?: () => void
  ) {
    const el = document.getElementById(containerId);
    if (!el) {
      throw new Error(`Container #${containerId} not found`);
    }
    this.container = el;
    this.onSaveCallback = onSave;
    this.onCloseCallback = onClose;
    this.render();
  }

  public async loadSettings(): Promise<StreamSettings> {
    try {
      const backendSettings = await invoke<StreamSettings>("get_stream_settings");
      if (backendSettings && backendSettings.resolution_mode) {
        this.currentSettings = backendSettings;
        localStorage.setItem(STORAGE_KEY, JSON.stringify(backendSettings));
        this.updateFormFields();
        this.updateDashboardQuickLabels();
        return backendSettings;
      }
    } catch (err) {
      console.warn("Could not query backend stream settings, falling back to localStorage:", err);
    }

    try {
      const stored = localStorage.getItem(STORAGE_KEY);
      if (stored) {
        const parsed = JSON.parse(stored) as StreamSettings;
        this.currentSettings = parsed;
        this.updateFormFields();
        this.updateDashboardQuickLabels();
        return parsed;
      }
    } catch (e) {
      console.warn("Error reading localStorage stream settings:", e);
    }

    this.currentSettings = { ...DEFAULT_STREAM_SETTINGS };
    this.updateFormFields();
    this.updateDashboardQuickLabels();
    return this.currentSettings;
  }

  public getSettings(): StreamSettings {
    return { ...this.currentSettings };
  }

  public open(): void {
    this.updateFormFields();
    this.clearMessages();
    this.container.classList.remove("hidden");
  }

  public close(): void {
    this.container.classList.add("hidden");
    this.clearMessages();
    this.onCloseCallback?.();
  }

  private clearMessages(): void {
    const errorEl = document.getElementById("settings-error-banner");
    const successEl = document.getElementById("settings-success-banner");
    if (errorEl) errorEl.classList.add("hidden");
    if (successEl) successEl.classList.add("hidden");
  }

  private showSuccess(msg: string): void {
    const successEl = document.getElementById("settings-success-banner");
    const errorEl = document.getElementById("settings-error-banner");
    if (errorEl) errorEl.classList.add("hidden");
    if (successEl) {
      successEl.textContent = msg;
      successEl.classList.remove("hidden");
    }
  }

  private showError(msg: string): void {
    const errorEl = document.getElementById("settings-error-banner");
    const successEl = document.getElementById("settings-success-banner");
    if (successEl) successEl.classList.add("hidden");
    if (errorEl) {
      errorEl.textContent = msg;
      errorEl.classList.remove("hidden");
    }
  }

  private updateDashboardQuickLabels(): void {
    const resEl = document.getElementById("quick-res-label");
    const fpsEl = document.getElementById("quick-fps-label");
    const brEl = document.getElementById("quick-bitrate-label");

    if (resEl) {
      const mode = this.currentSettings.resolution_mode;
      resEl.textContent = mode === "auto" ? "Auto" : mode.toUpperCase();
    }
    if (fpsEl) {
      fpsEl.textContent = `${this.currentSettings.framerate} FPS`;
    }
    if (brEl) {
      const mbps = Math.round(this.currentSettings.bitrate_kbps / 1000);
      brEl.textContent = `${mbps} Mbps`;
    }
  }

  private updateFormFields(): void {
    const resSelect = document.getElementById("settings-resolution-mode") as HTMLSelectElement | null;
    const customContainer = document.getElementById("settings-custom-res-container");
    const customWidth = document.getElementById("settings-custom-width") as HTMLInputElement | null;
    const customHeight = document.getElementById("settings-custom-height") as HTMLInputElement | null;
    const bitrateRange = document.getElementById("settings-bitrate-slider") as HTMLInputElement | null;
    const bitrateDisplay = document.getElementById("settings-bitrate-display");

    if (resSelect) {
      resSelect.value = this.currentSettings.resolution_mode;
    }

    if (customContainer) {
      if (this.currentSettings.resolution_mode === "custom") {
        customContainer.classList.remove("hidden");
      } else {
        customContainer.classList.add("hidden");
      }
    }

    if (customWidth) {
      customWidth.value = this.currentSettings.custom_width ? String(this.currentSettings.custom_width) : "1920";
    }
    if (customHeight) {
      customHeight.value = this.currentSettings.custom_height ? String(this.currentSettings.custom_height) : "1080";
    }

    // Framerate radio buttons
    const fpsRadio = document.querySelector(
      `input[name="settings-framerate"][value="${this.currentSettings.framerate}"]`
    ) as HTMLInputElement | null;
    if (fpsRadio) {
      fpsRadio.checked = true;
    }

    // Bitrate
    if (bitrateRange) {
      bitrateRange.value = String(this.currentSettings.bitrate_kbps);
    }
    if (bitrateDisplay) {
      const mbps = (this.currentSettings.bitrate_kbps / 1000).toFixed(1);
      bitrateDisplay.textContent = `${mbps} Mbps (${this.currentSettings.bitrate_kbps.toLocaleString()} kbps)`;
    }

    this.updateDashboardQuickLabels();
  }

  private render(): void {
    this.container.className =
      "fixed inset-0 bg-black/60 backdrop-blur-sm z-50 flex justify-end hidden transition-opacity duration-200";

    this.container.innerHTML = `
      <div class="w-full max-w-md bg-slate-900 border-l border-white/10 h-full p-6 flex flex-col shadow-2xl relative overflow-y-auto">
        <!-- Drawer Header -->
        <div class="flex items-center justify-between pb-4 border-b border-white/10 mb-5">
          <div class="flex items-center space-x-3">
            <div class="w-9 h-9 rounded-xl bg-violet-500/15 border border-violet-500/30 flex items-center justify-center text-violet-400">
              <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
              </svg>
            </div>
            <div>
              <h2 class="text-base font-semibold text-white">Stream & Display Settings</h2>
              <p class="text-xs text-slate-400">Low-latency video encoding preferences</p>
            </div>
          </div>
          <button id="settings-btn-close" class="text-slate-400 hover:text-white p-1 rounded-lg transition hover:bg-white/5">
            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/>
            </svg>
          </button>
        </div>

        <!-- Notification Banners -->
        <div id="settings-success-banner" class="hidden mb-4 p-3 bg-emerald-500/10 border border-emerald-500/20 text-emerald-400 text-xs rounded-xl flex items-center space-x-2">
          <svg class="w-4 h-4 text-emerald-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
          </svg>
          <span>Settings applied successfully</span>
        </div>
        <div id="settings-error-banner" class="hidden mb-4 p-3 bg-rose-500/10 border border-rose-500/20 text-rose-300 text-xs rounded-xl"></div>

        <!-- Form Body -->
        <form id="stream-settings-form" class="space-y-5 flex-1">
          <!-- Resolution Mode -->
          <div class="space-y-1.5">
            <label class="block text-xs font-medium text-slate-300">Display Resolution Mode</label>
            <select
              id="settings-resolution-mode"
              class="w-full px-3.5 py-2.5 rounded-xl bg-black/40 border border-white/10 text-white text-xs focus:outline-none focus:border-indigo-500 transition"
            >
              <option value="auto">Auto (Match Receiver Screen Geometry)</option>
              <option value="1080p">1080p Full HD (1920 × 1080)</option>
              <option value="1440p">1440p 2K QHD (2560 × 1440)</option>
              <option value="4k">4K Ultra HD (3840 × 2160)</option>
              <option value="custom">Custom Dimensions...</option>
            </select>
          </div>

          <!-- Custom Resolution Inputs -->
          <div id="settings-custom-res-container" class="hidden p-3.5 bg-black/30 rounded-xl border border-white/5 space-y-2">
            <div class="grid grid-cols-2 gap-2.5">
              <div>
                <label class="block text-[11px] text-slate-400 mb-1">Width (px)</label>
                <input
                  type="number"
                  id="settings-custom-width"
                  min="320"
                  max="7680"
                  class="w-full px-3 py-1.5 rounded-lg bg-black/40 border border-white/10 text-xs text-white"
                />
              </div>
              <div>
                <label class="block text-[11px] text-slate-400 mb-1">Height (px)</label>
                <input
                  type="number"
                  id="settings-custom-height"
                  min="240"
                  max="4320"
                  class="w-full px-3 py-1.5 rounded-lg bg-black/40 border border-white/10 text-xs text-white"
                />
              </div>
            </div>
          </div>

          <!-- Framerate -->
          <div class="space-y-2">
            <label class="block text-xs font-medium text-slate-300">Maximum Framerate (FPS)</label>
            <div class="grid grid-cols-3 gap-2">
              <label class="flex items-center justify-center p-2.5 rounded-xl border border-white/10 bg-black/30 cursor-pointer has-[:checked]:bg-indigo-600 has-[:checked]:border-indigo-500 has-[:checked]:text-white text-slate-300 text-xs font-medium transition">
                <input type="radio" name="settings-framerate" value="30" class="sr-only" />
                <span>30 FPS</span>
              </label>
              <label class="flex items-center justify-center p-2.5 rounded-xl border border-white/10 bg-black/30 cursor-pointer has-[:checked]:bg-indigo-600 has-[:checked]:border-indigo-500 has-[:checked]:text-white text-slate-300 text-xs font-medium transition">
                <input type="radio" name="settings-framerate" value="60" class="sr-only" checked />
                <span>60 FPS</span>
              </label>
              <label class="flex items-center justify-center p-2.5 rounded-xl border border-white/10 bg-black/30 cursor-pointer has-[:checked]:bg-indigo-600 has-[:checked]:border-indigo-500 has-[:checked]:text-white text-slate-300 text-xs font-medium transition">
                <input type="radio" name="settings-framerate" value="120" class="sr-only" />
                <span>120 FPS</span>
              </label>
            </div>
          </div>

          <!-- Video Bitrate -->
          <div class="space-y-2">
            <div class="flex justify-between items-center">
              <label class="text-xs font-medium text-slate-300">Target Video Bitrate</label>
              <span id="settings-bitrate-display" class="text-xs font-mono font-medium text-indigo-400">
                20.0 Mbps (20,000 kbps)
              </span>
            </div>
            <input
              type="range"
              id="settings-bitrate-slider"
              min="2000"
              max="50000"
              step="1000"
              value="20000"
              class="w-full accent-indigo-500 cursor-pointer"
            />
            <div class="flex justify-between text-[10px] text-slate-500 font-mono">
              <span>2 Mbps (Low Bandwidth)</span>
              <span>20 Mbps (Default)</span>
              <span>50 Mbps (Crisp Retina)</span>
            </div>
          </div>

          <!-- Hardware Acceleration Note -->
          <div class="p-3 rounded-xl bg-black/30 border border-white/5 space-y-1 text-xs text-slate-400">
            <div class="font-medium text-slate-300">Hardware Acceleration</div>
            <p class="text-[11px] leading-relaxed">
              Auto-detects Microsoft Media Foundation MFT on Windows and VA-API on Linux for sub-16ms encoding.
            </p>
          </div>

          <!-- Action Buttons -->
          <div class="pt-4 flex space-x-3">
            <button
              type="button"
              id="settings-btn-cancel"
              class="flex-1 py-2.5 text-xs bg-white/5 hover:bg-white/10 text-slate-300 hover:text-white rounded-xl border border-white/10 font-medium transition"
            >
              Cancel
            </button>
            <button
              type="submit"
              id="settings-btn-save"
              class="flex-1 py-2.5 text-xs bg-gradient-to-r from-indigo-600 to-violet-600 hover:from-indigo-500 hover:to-violet-500 text-white rounded-xl font-medium transition shadow-md shadow-indigo-950/50 flex items-center justify-center space-x-1.5"
            >
              <span>Save Settings</span>
            </button>
          </div>
        </form>
      </div>
    `;

    this.attachEventListeners();
  }

  private attachEventListeners(): void {
    const closeBtn = document.getElementById("settings-btn-close");
    const cancelBtn = document.getElementById("settings-btn-cancel");
    const form = document.getElementById("stream-settings-form");
    const resSelect = document.getElementById("settings-resolution-mode") as HTMLSelectElement | null;
    const customContainer = document.getElementById("settings-custom-res-container");
    const bitrateRange = document.getElementById("settings-bitrate-slider") as HTMLInputElement | null;
    const bitrateDisplay = document.getElementById("settings-bitrate-display");

    if (closeBtn) closeBtn.onclick = () => this.close();
    if (cancelBtn) cancelBtn.onclick = () => this.close();

    // Close on clicking backdrop
    this.container.onclick = (e) => {
      if (e.target === this.container) {
        this.close();
      }
    };

    if (resSelect && customContainer) {
      resSelect.onchange = () => {
        if (resSelect.value === "custom") {
          customContainer.classList.remove("hidden");
        } else {
          customContainer.classList.add("hidden");
        }
      };
    }

    if (bitrateRange && bitrateDisplay) {
      bitrateRange.oninput = () => {
        const val = parseInt(bitrateRange.value, 10);
        const mbps = (val / 1000).toFixed(1);
        bitrateDisplay.textContent = `${mbps} Mbps (${val.toLocaleString()} kbps)`;
      };
    }

    if (form) {
      form.onsubmit = async (e) => {
        e.preventDefault();
        if (this.isSaving) return;

        const resMode = resSelect ? resSelect.value : "auto";
        const fpsRadio = document.querySelector('input[name="settings-framerate"]:checked') as HTMLInputElement | null;
        const fps = fpsRadio ? parseInt(fpsRadio.value, 10) : 60;
        const bitrate = bitrateRange ? parseInt(bitrateRange.value, 10) : 20000;

        let customWidth: number | null = null;
        let customHeight: number | null = null;

        if (resMode === "custom") {
          const wInput = document.getElementById("settings-custom-width") as HTMLInputElement | null;
          const hInput = document.getElementById("settings-custom-height") as HTMLInputElement | null;
          customWidth = wInput ? parseInt(wInput.value, 10) : 1920;
          customHeight = hInput ? parseInt(hInput.value, 10) : 1080;

          if (isNaN(customWidth) || customWidth < 320 || customWidth > 7680) {
            this.showError("Custom width must be between 320 and 7680 px");
            return;
          }
          if (isNaN(customHeight) || customHeight < 240 || customHeight > 4320) {
            this.showError("Custom height must be between 240 and 4320 px");
            return;
          }
        }

        const newSettings: StreamSettings = {
          resolution_mode: resMode,
          custom_width: customWidth,
          custom_height: customHeight,
          framerate: fps,
          bitrate_kbps: bitrate,
        };

        this.isSaving = true;
        const saveBtn = document.getElementById("settings-btn-save") as HTMLButtonElement | null;
        if (saveBtn) {
          saveBtn.disabled = true;
          saveBtn.innerHTML = `
            <svg class="animate-spin -ml-1 mr-1.5 h-3 w-3 text-white inline" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
              <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
              <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8H4z"></path>
            </svg>
            Saving...
          `;
        }

        try {
          await invoke("set_stream_settings", { settings: newSettings });
          if (this.onSaveCallback) {
            await this.onSaveCallback(newSettings);
          }
          this.currentSettings = newSettings;
          localStorage.setItem(STORAGE_KEY, JSON.stringify(newSettings));
          this.updateDashboardQuickLabels();
          this.showSuccess("Settings applied successfully!");
          setTimeout(() => this.close(), 700);
        } catch (err: any) {
          this.showError(`Failed to save settings: ${err?.message || err}`);
        } finally {
          this.isSaving = false;
          if (saveBtn) {
            saveBtn.disabled = false;
            saveBtn.textContent = "Save Settings";
          }
        }
      };
    }
  }
}
