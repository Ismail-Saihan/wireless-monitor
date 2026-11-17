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
        return parsed;
      }
    } catch (e) {
      console.warn("Error reading localStorage stream settings:", e);
    }

    this.currentSettings = { ...DEFAULT_STREAM_SETTINGS };
    this.updateFormFields();
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
      setTimeout(() => {
        successEl.classList.add("hidden");
      }, 3000);
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

  private render(): void {
    this.container.className = "fixed inset-0 z-50 overflow-hidden hidden";

    this.container.innerHTML = `
      <!-- Backdrop -->
      <div id="settings-backdrop" class="fixed inset-0 bg-slate-950/70 backdrop-blur-sm transition-opacity"></div>

      <!-- Slide-out Drawer Panel -->
      <div class="fixed inset-y-0 right-0 max-w-md w-full bg-slate-900 border-l border-slate-800 p-6 flex flex-col justify-between shadow-2xl z-10 overflow-y-auto">
        <div class="space-y-6">
          <!-- Header -->
          <div class="flex items-center justify-between border-b border-slate-800 pb-4">
            <div class="flex items-center space-x-2.5">
              <div class="w-8 h-8 rounded-xl bg-indigo-600/20 text-indigo-400 flex items-center justify-center font-bold text-sm border border-indigo-500/20">
                ⚙️
              </div>
              <div>
                <h2 class="text-sm font-semibold text-white">Stream & Display Settings</h2>
                <p class="text-xs text-slate-400">Configure resolution, framerate, and network bitrate</p>
              </div>
            </div>
            <button id="settings-btn-close" class="text-slate-400 hover:text-slate-200 text-xl leading-none p-1 transition">
              &times;
            </button>
          </div>

          <!-- Notification Banners -->
          <div id="settings-error-banner" class="hidden p-3 rounded-xl bg-rose-500/10 border border-rose-500/20 text-rose-400 text-xs font-medium"></div>
          <div id="settings-success-banner" class="hidden p-3 rounded-xl bg-emerald-500/10 border border-emerald-500/20 text-emerald-400 text-xs font-medium"></div>

          <!-- Settings Form -->
          <div class="space-y-5">
            <!-- Resolution Mode -->
            <div>
              <label class="block text-xs font-semibold text-slate-300 uppercase tracking-wider mb-2">
                Resolution Mode
              </label>
              <div class="space-y-2" id="resolution-options-group">
                <label class="flex items-start space-x-3 p-3 bg-slate-950/60 rounded-xl border border-slate-800 cursor-pointer hover:border-slate-700 transition">
                  <input type="radio" name="resolution_mode" value="auto" class="mt-0.5 text-indigo-600 focus:ring-indigo-500" />
                  <div>
                    <div class="text-xs font-medium text-white">Auto (Match Client Display)</div>
                    <div class="text-[11px] text-slate-400 mt-0.5">Dynamically queries and adopts the connected client's native screen dimensions.</div>
                  </div>
                </label>

                <label class="flex items-start space-x-3 p-3 bg-slate-950/60 rounded-xl border border-slate-800 cursor-pointer hover:border-slate-700 transition">
                  <input type="radio" name="resolution_mode" value="1080p" class="mt-0.5 text-indigo-600 focus:ring-indigo-500" />
                  <div>
                    <div class="text-xs font-medium text-white">1080p Full HD (1920 × 1080)</div>
                    <div class="text-[11px] text-slate-400 mt-0.5">Standard 16:9 widescreen streaming. Ideal for standard monitors and laptops.</div>
                  </div>
                </label>

                <label class="flex items-start space-x-3 p-3 bg-slate-950/60 rounded-xl border border-slate-800 cursor-pointer hover:border-slate-700 transition">
                  <input type="radio" name="resolution_mode" value="1440p" class="mt-0.5 text-indigo-600 focus:ring-indigo-500" />
                  <div>
                    <div class="text-xs font-medium text-white">1440p 2K QHD (2560 × 1440)</div>
                    <div class="text-[11px] text-slate-400 mt-0.5">Crisp high-density resolution for modern monitors and tablets.</div>
                  </div>
                </label>

                <label class="flex items-start space-x-3 p-3 bg-slate-950/60 rounded-xl border border-slate-800 cursor-pointer hover:border-slate-700 transition">
                  <input type="radio" name="resolution_mode" value="4k" class="mt-0.5 text-indigo-600 focus:ring-indigo-500" />
                  <div>
                    <div class="text-xs font-medium text-white">4K Ultra HD (3840 × 2160)</div>
                    <div class="text-[11px] text-slate-400 mt-0.5">Maximum fidelity. Requires robust Wi-Fi 6 or Gigabit LAN bandwidth.</div>
                  </div>
                </label>

                <label class="flex items-start space-x-3 p-3 bg-slate-950/60 rounded-xl border border-slate-800 cursor-pointer hover:border-slate-700 transition">
                  <input type="radio" name="resolution_mode" value="custom" class="mt-0.5 text-indigo-600 focus:ring-indigo-500" />
                  <div class="flex-1">
                    <div class="text-xs font-medium text-white">Custom Dimensions</div>
                    <div class="text-[11px] text-slate-400 mt-0.5">Specify explicit pixel width and height.</div>
                    
                    <!-- Custom Dimensions Input Row (shown when custom is selected) -->
                    <div id="custom-resolution-fields" class="hidden mt-3 pt-3 border-t border-slate-800 grid grid-cols-2 gap-3">
                      <div>
                        <label class="block text-[11px] text-slate-400 mb-1">Width (px)</label>
                        <input
                          id="settings-custom-width"
                          type="number"
                          min="640"
                          max="7680"
                          step="2"
                          placeholder="1920"
                          class="w-full text-xs font-mono px-3 py-1.5 bg-slate-900 border border-slate-700 rounded-lg text-white focus:outline-none focus:ring-1 focus:ring-indigo-500"
                        />
                      </div>
                      <div>
                        <label class="block text-[11px] text-slate-400 mb-1">Height (px)</label>
                        <input
                          id="settings-custom-height"
                          type="number"
                          min="480"
                          max="4320"
                          step="2"
                          placeholder="1080"
                          class="w-full text-xs font-mono px-3 py-1.5 bg-slate-900 border border-slate-700 rounded-lg text-white focus:outline-none focus:ring-1 focus:ring-indigo-500"
                        />
                      </div>
                    </div>
                  </div>
                </label>
              </div>
            </div>

            <!-- Framerate -->
            <div>
              <label class="block text-xs font-semibold text-slate-300 uppercase tracking-wider mb-2">
                Target Framerate
              </label>
              <div class="grid grid-cols-2 gap-3">
                <label class="flex items-center space-x-2.5 p-3 bg-slate-950/60 rounded-xl border border-slate-800 cursor-pointer hover:border-slate-700 transition">
                  <input type="radio" name="framerate" value="60" class="text-indigo-600 focus:ring-indigo-500" />
                  <div>
                    <div class="text-xs font-medium text-white">60 FPS</div>
                    <div class="text-[10px] text-emerald-400 mt-0.5">Fluid Motion (Default)</div>
                  </div>
                </label>

                <label class="flex items-center space-x-2.5 p-3 bg-slate-950/60 rounded-xl border border-slate-800 cursor-pointer hover:border-slate-700 transition">
                  <input type="radio" name="framerate" value="30" class="text-indigo-600 focus:ring-indigo-500" />
                  <div>
                    <div class="text-xs font-medium text-white">30 FPS</div>
                    <div class="text-[10px] text-slate-400 mt-0.5">Eco / Low Bandwidth</div>
                  </div>
                </label>
              </div>
            </div>

            <!-- Bitrate Cap -->
            <div>
              <label class="block text-xs font-semibold text-slate-300 uppercase tracking-wider mb-2">
                Video Bitrate Cap
              </label>
              <div class="space-y-2">
                <label class="flex items-center space-x-3 p-3 bg-slate-950/60 rounded-xl border border-slate-800 cursor-pointer hover:border-slate-700 transition">
                  <input type="radio" name="bitrate_kbps" value="10000" class="text-indigo-600 focus:ring-indigo-500" />
                  <div class="flex-1 flex items-center justify-between">
                    <div>
                      <div class="text-xs font-medium text-white">10 Mbps</div>
                      <div class="text-[11px] text-slate-400">Wi-Fi Eco (Saves power & battery)</div>
                    </div>
                    <span class="text-[10px] px-2 py-0.5 rounded-full bg-slate-800 text-slate-300 border border-slate-700 font-mono">10,000 kbps</span>
                  </div>
                </label>

                <label class="flex items-center space-x-3 p-3 bg-slate-950/60 rounded-xl border border-slate-800 cursor-pointer hover:border-slate-700 transition">
                  <input type="radio" name="bitrate_kbps" value="20000" class="text-indigo-600 focus:ring-indigo-500" />
                  <div class="flex-1 flex items-center justify-between">
                    <div>
                      <div class="text-xs font-medium text-white">20 Mbps</div>
                      <div class="text-[11px] text-indigo-300">Balanced (Recommended)</div>
                    </div>
                    <span class="text-[10px] px-2 py-0.5 rounded-full bg-indigo-500/10 text-indigo-300 border border-indigo-500/20 font-mono">20,000 kbps</span>
                  </div>
                </label>

                <label class="flex items-center space-x-3 p-3 bg-slate-950/60 rounded-xl border border-slate-800 cursor-pointer hover:border-slate-700 transition">
                  <input type="radio" name="bitrate_kbps" value="40000" class="text-indigo-600 focus:ring-indigo-500" />
                  <div class="flex-1 flex items-center justify-between">
                    <div>
                      <div class="text-xs font-medium text-white">40 Mbps</div>
                      <div class="text-[11px] text-emerald-400">High Quality LAN (Gigabit/5GHz)</div>
                    </div>
                    <span class="text-[10px] px-2 py-0.5 rounded-full bg-emerald-500/10 text-emerald-300 border border-emerald-500/20 font-mono">40,000 kbps</span>
                  </div>
                </label>
              </div>
            </div>
          </div>
        </div>

        <!-- Footer Actions -->
        <div class="pt-6 border-t border-slate-800 flex items-center space-x-3">
          <button id="settings-btn-reset" class="px-3 py-2 text-xs text-slate-400 hover:text-slate-200 transition font-medium">
            Reset Defaults
          </button>
          <div class="flex-1 flex items-center justify-end space-x-2">
            <button id="settings-btn-cancel" class="px-3.5 py-2 text-xs bg-slate-800 hover:bg-slate-750 text-slate-300 rounded-xl font-medium border border-slate-700/60 transition">
              Cancel
            </button>
            <button id="settings-btn-save" class="px-4 py-2 text-xs bg-indigo-600 hover:bg-indigo-500 text-white rounded-xl font-medium transition shadow-lg shadow-indigo-600/30 flex items-center space-x-1.5">
              <span>Save Settings</span>
            </button>
          </div>
        </div>
      </div>
    `;

    // Hook listeners
    const backdrop = document.getElementById("settings-backdrop");
    const closeBtn = document.getElementById("settings-btn-close");
    const cancelBtn = document.getElementById("settings-btn-cancel");
    const resetBtn = document.getElementById("settings-btn-reset");
    const saveBtn = document.getElementById("settings-btn-save");

    if (backdrop) backdrop.onclick = () => this.close();
    if (closeBtn) closeBtn.onclick = () => this.close();
    if (cancelBtn) cancelBtn.onclick = () => this.close();

    if (resetBtn) {
      resetBtn.onclick = () => {
        this.currentSettings = { ...DEFAULT_STREAM_SETTINGS };
        this.updateFormFields();
      };
    }

    if (saveBtn) {
      saveBtn.onclick = () => this.handleSave();
    }

    // Toggle custom fields visibility on resolution mode radio change
    const radios = this.container.querySelectorAll<HTMLInputElement>('input[name="resolution_mode"]');
    radios.forEach((r) => {
      r.onchange = () => {
        this.toggleCustomFields(r.value === "custom");
      };
    });
  }

  private toggleCustomFields(show: boolean): void {
    const customFields = document.getElementById("custom-resolution-fields");
    if (customFields) {
      if (show) {
        customFields.classList.remove("hidden");
      } else {
        customFields.classList.add("hidden");
      }
    }
  }

  private updateFormFields(): void {
    // 1. Resolution Mode
    const resRadios = this.container.querySelectorAll<HTMLInputElement>('input[name="resolution_mode"]');
    resRadios.forEach((r) => {
      r.checked = r.value === this.currentSettings.resolution_mode;
    });

    const isCustom = this.currentSettings.resolution_mode === "custom";
    this.toggleCustomFields(isCustom);

    const widthInput = document.getElementById("settings-custom-width") as HTMLInputElement;
    const heightInput = document.getElementById("settings-custom-height") as HTMLInputElement;
    if (widthInput && this.currentSettings.custom_width) {
      widthInput.value = this.currentSettings.custom_width.toString();
    }
    if (heightInput && this.currentSettings.custom_height) {
      heightInput.value = this.currentSettings.custom_height.toString();
    }

    // 2. Framerate
    const fpsRadios = this.container.querySelectorAll<HTMLInputElement>('input[name="framerate"]');
    fpsRadios.forEach((r) => {
      r.checked = parseInt(r.value, 10) === this.currentSettings.framerate;
    });

    // 3. Bitrate
    const bitrateRadios = this.container.querySelectorAll<HTMLInputElement>('input[name="bitrate_kbps"]');
    bitrateRadios.forEach((r) => {
      r.checked = parseInt(r.value, 10) === this.currentSettings.bitrate_kbps;
    });
  }

  private async handleSave(): Promise<void> {
    if (this.isSaving) return;

    // Read selected resolution mode
    const selectedRes = this.container.querySelector<HTMLInputElement>('input[name="resolution_mode"]:checked');
    const resolutionMode = selectedRes ? selectedRes.value : "auto";

    let customWidth: number | null = null;
    let customHeight: number | null = null;

    if (resolutionMode === "custom") {
      const widthInput = document.getElementById("settings-custom-width") as HTMLInputElement;
      const heightInput = document.getElementById("settings-custom-height") as HTMLInputElement;

      const w = widthInput ? parseInt(widthInput.value, 10) : NaN;
      const h = heightInput ? parseInt(heightInput.value, 10) : NaN;

      if (isNaN(w) || isNaN(h)) {
        this.showError("Custom width and height are required");
        return;
      }
      if (w % 2 !== 0 || h % 2 !== 0) {
        this.showError("Custom width and height must be even numbers");
        return;
      }
      if (w < 640 || w > 7680) {
        this.showError("Custom width must be between 640 and 7680");
        return;
      }
      if (h < 480 || h > 4320) {
        this.showError("Custom height must be between 480 and 4320");
        return;
      }
      customWidth = w;
      customHeight = h;
    }

    // Read selected framerate
    const selectedFps = this.container.querySelector<HTMLInputElement>('input[name="framerate"]:checked');
    const framerate = selectedFps ? parseInt(selectedFps.value, 10) : 60;

    // Read selected bitrate
    const selectedBitrate = this.container.querySelector<HTMLInputElement>('input[name="bitrate_kbps"]:checked');
    const bitrateKbps = selectedBitrate ? parseInt(selectedBitrate.value, 10) : 20000;

    const newSettings: StreamSettings = {
      resolution_mode: resolutionMode,
      custom_width: customWidth,
      custom_height: customHeight,
      framerate,
      bitrate_kbps: bitrateKbps,
    };

    this.isSaving = true;
    const saveBtn = document.getElementById("settings-btn-save");
    if (saveBtn) {
      saveBtn.innerHTML = `
        <svg class="animate-spin -ml-1 mr-1.5 h-3.5 w-3.5 text-white inline" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
          <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
          <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8H4z"></path>
        </svg>
        Saving...
      `;
    }

    try {
      if (this.onSaveCallback) {
        await this.onSaveCallback(newSettings);
      } else {
        await invoke("set_stream_settings", { settings: newSettings });
      }

      this.currentSettings = newSettings;
      localStorage.setItem(STORAGE_KEY, JSON.stringify(newSettings));
      this.showSuccess("Settings saved successfully!");
      setTimeout(() => {
        this.close();
      }, 700);
    } catch (err: any) {
      const errMsg = typeof err === "string" ? err : err.message || "Failed to save settings";
      this.showError(errMsg);
    } finally {
      this.isSaving = false;
      if (saveBtn) {
        saveBtn.innerHTML = "<span>Save Settings</span>";
      }
    }
  }
}
