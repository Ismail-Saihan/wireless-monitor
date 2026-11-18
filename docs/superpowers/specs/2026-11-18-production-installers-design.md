# Design Document: Production Turnkey Installers for Windows and Linux

**Date:** 2026-11-18  
**Status:** Approved for Implementation  
**Target Repository:** `f:\Wireless Monitor Project`  

---

## 1. Intent & The "Why"

Users expect desktop applications to be installable through conventional, familiar operating system installation mechanisms:
- On **Windows**: A standard double-click Setup Wizard executable (`.exe` via NSIS) or Windows Installer package (`.msi` via WiX) that installs the application to `Program Files` or per-user local app data, registers Start Menu and Desktop shortcuts, stages companion drivers, and registers a standard entry in Windows "Installed Apps" / "Add or Remove Programs" with a clean uninstaller.
- On **Linux**: A native Debian package (`.deb`) installable via `sudo apt install ./...deb` or graphical package managers (Ubuntu Software, Discover), registering a freedesktop `.desktop` launcher entry with system categories and icons, or a single-file executable `.AppImage` requiring zero installation.

Currently, the repository has foundational bundling definitions in `src-tauri/tauri.conf.json`, but lacks application icon assets (`icons/` is missing, causing installer failures), has incomplete installer hook scripts and desktop shortcuts, lacks runtime installed resource resolution in `windows_iddcx.rs` (so installed apps cannot locate driver scripts in `Program Files`), has no 1-click in-app driver elevation workflow, and lacks automated cross-platform CI/CD release pipelines (`.github/workflows/release.yml`).

This design delivers full end-to-end production packaging and installation polish for both Windows and Linux.

---

## 2. Goals, Non-Goals & Anti-Goals

### 2.1 Goals
- **Application Icon Suite:**
  - Generate crisp, multi-resolution application icons adhering to platform standards:
    - `src-tauri/icons/icon.ico` (Windows executable & installer icon with 16, 32, 48, 64, 128, 256px mipmaps).
    - `src-tauri/icons/32x32.png` and `128x128.png` (Linux desktop icons).
    - `src-tauri/icons/128x128@2x.png` (256x256 high-DPI icon).
    - `src-tauri/icons/icon.png` (512x512 master application icon).
  - Map icon paths into `src-tauri/tauri.conf.json`.
- **Windows NSIS & MSI Turnkey Packaging:**
  - Configure NSIS installer:
    - Mode: `both` (supports both Per-Machine `/allusers` with UAC and Current-User install).
    - Desktop and Start Menu shortcut creation.
    - Driver staging bundled into installed directory (`<InstallDir>/drivers/windows/`).
    - Standard Uninstaller registering with Windows Add/Remove Programs.
  - Configure MSI (WiX) enterprise installer target.
- **Linux DEB & AppImage Turnkey Packaging:**
  - Configure `.deb` package:
    - Installs binary to `/usr/bin/wireless-monitor`.
    - Installs freedesktop `.desktop` entry to `/usr/share/applications/` with `Categories=Utility;Network;Video;`.
    - Installs desktop icon to `/usr/share/icons/hicolor/512x512/apps/wireless-monitor.png`.
    - Declares runtime dependencies: `libpipewire-0.3-0`, `libdbus-1-3`, `libxdo3`, `libwebkit2gtk-4.1-0 | libwebkit2gtk-4.0-37`.
  - Configure AppImage bundling.
- **Installed Runtime Driver Path Resolution:**
  - Update `WindowsIddCxManager::resolve_script_path` to probe paths relative to `std::env::current_exe()`, ensuring installed binaries in `C:\Program Files\Wireless Monitor\` find `drivers/windows/install_driver.bat`.
- **UI 1-Click Driver Installation Banner:**
  - In `StatusHeader.ts` / Dashboard, when running on Windows in Host role:
    - Query `is_driver_installed`.
    - If not installed, render a banner with a 1-click **"Install Driver (Host Mode)"** button that executes the elevated UAC prompt via Tauri command `install_driver` without requiring manual command lines.
    - Real-time status update to "Driver Installed & Ready" upon success.
- **Cross-Platform Release Automation:**
  - Provide `.github/workflows/release.yml` automating builds on `windows-latest` (producing `.exe` and `.msi`) and `ubuntu-22.04` (producing `.deb` and `.AppImage`), uploading all 4 artifacts on release.
  - Provide local build helper scripts: `scripts/build-windows.ps1` and `scripts/build-linux.sh`.
- **User Installation Guide:**
  - Provide `INSTALL.md` with clear, normal user install workflows for Windows and Linux.
- **Automated Verification Suite:**
  - Provide `src-tauri/tests/verify_installers.py` verifying icon structure, bundle configurations, runtime path resolution logic, and documentation.

### 2.2 Non-Goals
- Proprietary app store submissions (Microsoft Store MSIX or Canonical Snapcraft).
- Kernel-mode driver signing with EV hardware tokens (retains test-signing certificate `.cer` with automated certmgr registration).

### 2.3 Anti-Goals
- Requiring manual command-line execution or script manipulation by standard end-users.
- Silent driver installation without explicit UAC consent.
- Leaving undeleted driver remnants or registry keys upon application uninstallation.

---

## 3. Technical Architecture & File Layout

```
f:\Wireless Monitor Project\
├── .github/
│   └── workflows/
│       └── release.yml                 <-- Automated multi-platform CI/CD builder
├── scripts/
│   ├── generate_icons.py               <-- High-resolution icon synthesis script
│   ├── build-windows.ps1               <-- Local Windows installer build automation
│   └── build-linux.sh                  <-- Local Linux package build automation
├── src-tauri/
│   ├── icons/
│   │   ├── icon.ico                    <-- Multi-resolution Windows icon
│   │   ├── icon.png                    <-- 512x512 master icon
│   │   ├── 32x32.png                   <-- Taskbar / small icon
│   │   ├── 128x128.png                 <-- Standard Linux icon
│   │   └── 128x128@2x.png              <-- High-DPI icon
│   ├── tauri.conf.json                 <-- Updated NSIS, MSI, DEB, AppImage configs
│   ├── src/
│   │   └── display/
│   │       └── windows_iddcx.rs        <-- current_exe relative resource resolution
│   └── tests/
│       ├── installer_packaging_test.rs <-- Rust unit test for packaging contracts
│       └── verify_installers.py        <-- Automated verification suite
├── ui/
│   └── src/
│       └── components/
│           └── StatusHeader.ts         <-- 1-click driver installation banner & action
└── INSTALL.md                          <-- User-facing installation manual
```

---

## 4. Verification & Quality Gates

1. Icon dimensions, formats, and headers must match platform standards (valid ICO and PNG magic numbers).
2. `tauri.conf.json` must validate against Tauri 2.0 schema without warnings or rejected properties.
3. Path resolution in `WindowsIddCxManager` must resolve successfully from both development workspace and installed Program Files layouts.
4. Driver installation banner must accurately reflect driver installation state and trigger `install_driver`.
5. Automated test suite `verify_installers.py` must run and pass with 0 errors.
6. All existing Phase 1, Phase 2, and Phase 3 regression tests must pass without regressions.
