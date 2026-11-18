# Production Turnkey Installers for Windows and Linux Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver turnkey, normal installation packages (NSIS `.exe` and MSI `.msi` on Windows, Debian `.deb` and AppImage on Linux) with multi-resolution application icons, installed-path driver resource resolution, 1-click in-app driver elevation UI, automated build scripts, GitHub Actions release CI/CD, and user installation documentation.

**Architecture:** Extend Tauri 2.0 bundle configuration with production installer options for NSIS and Linux DEB/AppImage; synthesize standard multi-resolution icons using Pillow; update `WindowsIddCxManager` to dynamically locate driver installation assets relative to `std::env::current_exe()`; embed a 1-click driver installation prompt in the UI header for seamless Windows host onboarding; provide turnkey cross-platform release automation in GitHub Actions.

**Tech Stack:** Rust (Tauri 2.0 bundler, `windows-rs`), TypeScript, Vite, Python (Pillow for icon synthesis), PowerShell, Bash, GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-11-18-production-installers-design.md`

## Global Constraints

- Must generate valid Windows `.ico` and Linux `.png` icon files matching exact expected mipmap resolutions.
- `src-tauri/tauri.conf.json` must be strictly compliant with Tauri 2.0 schema without invalid root or bundle attributes.
- Driver resolution in `WindowsIddCxManager` must work in both development workspace (`drivers/windows/`) and installed Program Files directory (`<current_exe>/drivers/windows/` or `<current_exe>/resources/drivers/windows/`).
- Zero regressions across existing Phase 1, Phase 2, and Phase 3 verification suites.

## Review Focus

- Missing icon file paths breaking `tauri build` packaging.
- Absolute or invalid paths in `resolve_script_path` failing when application is installed in `C:\Program Files\Wireless Monitor\`.
- Failure to handle UAC elevation cancellation gracefully in UI `install_driver`.
- Linux Debian package missing necessary desktop environment categories or required shared library dependencies.
- GitHub Actions workflow missing required build dependencies on Ubuntu or Windows runners.

---

### Task 1: Application Icon Assets Generation & Tauri Manifest Configuration

**Files:**
- Create: `scripts/generate_icons.py`
- Create: `src-tauri/icons/icon.ico`
- Create: `src-tauri/icons/icon.png`
- Create: `src-tauri/icons/32x32.png`
- Create: `src-tauri/icons/128x128.png`
- Create: `src-tauri/icons/128x128@2x.png`
- Modify: `src-tauri/tauri.conf.json`

**Interfaces:**
- Produces: Standard multi-resolution icon assets in `src-tauri/icons/` referenced by `bundle.icon` in `tauri.conf.json`.

- [ ] **Step 1: Write `scripts/generate_icons.py` to synthesize high-res icons**
Create script using Python Pillow that generates a modern monitor + wireless signal badge icon and outputs all required dimensions: 32x32, 128x128, 256x256 (`128x128@2x`), 512x512 (`icon.png`), and multi-mipmap `icon.ico`.

- [ ] **Step 2: Run `python scripts/generate_icons.py`**
Run: `python scripts/generate_icons.py`
Expected: Output files created in `src-tauri/icons/`.

- [ ] **Step 3: Update `src-tauri/tauri.conf.json` with icon mappings**
Configure `bundle.icon` array with `["icons/32x32.png", "icons/128x128.png", "icons/128x128@2x.png", "icons/icon.ico", "icons/icon.png"]`.

- [ ] **Step 4: Verify icon files exist and are non-empty**
Run: `python -c "import os; [os.path.getsize(f'src-tauri/icons/{f}') for f in ['icon.ico', '32x32.png', '128x128.png', '128x128@2x.png', 'icon.png']]"`
Expected: All files exist with non-zero byte size.

- [ ] **Step 5: Commit Task 1**
```bash
git add scripts/generate_icons.py src-tauri/icons/ src-tauri/tauri.conf.json
git commit -m "feat(icons): generate multi-resolution application icons and map in tauri.conf.json"
```

---

### Task 2: Runtime Installed Resource Resolution in `windows_iddcx.rs`

**Files:**
- Modify: `src-tauri/src/display/windows_iddcx.rs`
- Create: `src-tauri/tests/installer_packaging_test.rs`

**Interfaces:**
- Consumes: `std::env::current_exe()`
- Produces: `WindowsIddCxManager::resolve_script_path` supporting installed application directories.

- [ ] **Step 1: Write unit test in `src-tauri/tests/installer_packaging_test.rs`**
Add test asserting that `resolve_script_path` resolves candidates relative to executable parent directories.

- [ ] **Step 2: Run test to observe behavior**
Run: `python -c "import subprocess; ..."` / Rust test check.

- [ ] **Step 3: Enhance `resolve_script_path` in `src-tauri/src/display/windows_iddcx.rs`**
Update candidate search to probe:
1. Direct existence of `script`.
2. Workspace relative paths (`drivers/windows`, `../drivers/windows`, `../../drivers/windows`).
3. Executable parent relative paths (`parent/drivers/windows`, `parent/resources/drivers/windows`, `parent/_up_/drivers/windows`).

- [ ] **Step 4: Verify syntax and behavior**
Run: `python src-tauri/tests/verify_display.py`
Expected: PASS with 0 errors.

- [ ] **Step 5: Commit Task 2**
```bash
git add src-tauri/src/display/windows_iddcx.rs src-tauri/tests/installer_packaging_test.rs
git commit -m "feat(display): support runtime installed resource resolution in WindowsIddCxManager"
```

---

### Task 3: 1-Click Driver Installation Banner & Action in UI Dashboard

**Files:**
- Modify: `ui/src/components/StatusHeader.ts`
- Modify: `ui/src/main.ts`

**Interfaces:**
- Consumes: Tauri IPC command `install_driver`
- Produces: Interactive driver status indicator and 1-click install button in dashboard when running as Windows Host without driver.

- [ ] **Step 1: Add driver installation status and 1-click trigger to `StatusHeader.ts`**
Check `phase2Status.windows_receiver_ready` and operating system. When Windows Host has no driver, display an actionable notice with button: "Install Display Driver".

- [ ] **Step 2: Wire `onInstallDriver` handler in `ui/src/main.ts`**
Invoke `invoke('install_driver')`, display confirmation toast/alert, and refresh status.

- [ ] **Step 3: Typecheck frontend TypeScript**
Run: `npx tsc --noEmit && pnpm run build`
Expected: 0 errors, successful Vite build.

- [ ] **Step 4: Commit Task 3**
```bash
git add ui/src/components/StatusHeader.ts ui/src/main.ts
git commit -m "feat(ui): add 1-click driver installation banner and action for Windows Host"
```

---

### Task 4: Windows NSIS & Linux Packaging Configurations & Build Scripts

**Files:**
- Modify: `src-tauri/tauri.conf.json`
- Create: `scripts/build-windows.ps1`
- Create: `scripts/build-linux.sh`

**Interfaces:**
- Produces: Fully tuned `tauri.conf.json` NSIS and Linux bundling settings and turnkey local build scripts.

- [ ] **Step 1: Tune NSIS and Linux settings in `src-tauri/tauri.conf.json`**
Configure NSIS `installMode: "both"`, shortcuts, and Linux `categories: ["Utility", "Network", "AudioVideo"]`.

- [ ] **Step 2: Create `scripts/build-windows.ps1`**
Create automated PowerShell script to build frontend, check Rust toolchain, and execute `pnpm tauri build --target nsis,msi`.

- [ ] **Step 3: Create `scripts/build-linux.sh`**
Create automated Bash script to build frontend and execute `pnpm tauri build --target deb,appimage`.

- [ ] **Step 4: Validate packaging test**
Run: `python src-tauri/tests/verify_packaging.py`
Expected: PASS with 0 errors.

- [ ] **Step 5: Commit Task 4**
```bash
git add src-tauri/tauri.conf.json scripts/build-windows.ps1 scripts/build-linux.sh
git commit -m "feat(packaging): tune NSIS/Linux bundling configurations and provide build scripts"
```

---

### Task 5: Automated GitHub Actions Cross-Platform Release Workflow

**Files:**
- Create: `.github/workflows/release.yml`

**Interfaces:**
- Produces: GitHub Actions workflow triggered on tag pushes (`v*`) or manual `workflow_dispatch`, building Windows (`.exe`, `.msi`) and Linux (`.deb`, `.AppImage`) installers and publishing them to GitHub Releases.

- [ ] **Step 1: Create `.github/workflows/release.yml`**
Define matrix build for `windows-latest` and `ubuntu-22.04` with all required system libraries (`libpipewire-0.3-dev`, `libwebkit2gtk-4.1-dev`, `libappindicator3-dev`), Node 20, Rust stable, and Tauri CLI.

- [ ] **Step 2: Validate workflow YAML syntax**
Run Python YAML parser to ensure clean formatting.

- [ ] **Step 3: Commit Task 5**
```bash
git add .github/workflows/release.yml
git commit -m "ci: add cross-platform GitHub Actions release workflow for Windows and Linux installers"
```

---

### Task 6: User-Facing Installation Guide & Documentation

**Files:**
- Create: `INSTALL.md`
- Modify: `README.md`
- Modify: `AGENT.md`

**Interfaces:**
- Produces: Clear, step-by-step user installation instructions for both Windows (`.exe` / `.msi`) and Linux (`.deb` / `.AppImage`), plus uninstallation procedures.

- [ ] **Step 1: Create `INSTALL.md`**
Document standard installation process for Windows and Linux with screenshots/step explanations.

- [ ] **Step 2: Update `README.md` and `AGENT.md`**
Add prominent links to `INSTALL.md` and document installer build workflows.

- [ ] **Step 3: Commit Task 6**
```bash
git add INSTALL.md README.md AGENT.md
git commit -m "docs: add user installation guide and update README and AGENT guidelines"
```

---

### Task 7: Comprehensive Verification Suite & Zero-Regression Check

**Files:**
- Create: `src-tauri/tests/verify_installers.py`

**Interfaces:**
- Produces: Comprehensive automated test suite validating icons, installer configurations, path resolution logic, build scripts, and documentation.

- [ ] **Step 1: Write `src-tauri/tests/verify_installers.py`**
Implement checks for icon assets, ICO binary header, PNG dimensions, tauri.conf.json bundle schema, resolve_script_path logic, GitHub Actions workflow, and documentation.

- [ ] **Step 2: Run `verify_installers.py`**
Run: `python src-tauri/tests/verify_installers.py`
Expected: ALL CHECKS PASS (0 errors).

- [ ] **Step 3: Run all existing verification test suites**
Run: `python src-tauri/tests/verify_phase3.py && python src-tauri/tests/verify_phase2.py && python src-tauri/tests/verify_e2e.py`
Expected: ALL SUITES PASS without regression.

- [ ] **Step 4: Commit Task 7**
```bash
git add src-tauri/tests/verify_installers.py
git commit -m "test: add verify_installers suite and confirm zero regressions across all test suites"
```
