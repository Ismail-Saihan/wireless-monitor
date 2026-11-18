# Installation Guide: Wireless Virtual Monitor 🖥️📡

Welcome to **Wireless Virtual Monitor**! This guide details the standard, normal installation process for both **Windows** and **Linux** systems, enabling you to use your PC or laptop as an authentic wireless secondary display.

Pre-built binaries and installers can be downloaded directly from the **[GitHub Releases Page](https://github.com/Ismail-Saihan/wireless-monitor/releases)**.

---

## 🪟 Windows Installation

We provide three Windows distribution formats:
1. **Portable Standalone Binary (`wireless-monitor.exe`)** — *Zero installation required; runs immediately.*
2. **NSIS Setup Wizard (`.exe`)** — *Recommended for standard desktop installation with shortcuts.*
3. **MSI Enterprise Installer (`.msi`)** — *Recommended for enterprise and IT deployments.*

### Method 1: Portable Standalone Executable (`wireless-monitor.exe`)

1. **Download:** Grab `wireless-monitor.exe` from [GitHub Releases](https://github.com/Ismail-Saihan/wireless-monitor/releases) or locate it in the project root directory.
2. **Run:** Double-click `wireless-monitor.exe`.
3. **No Setup Wizard Required:** The modern minimal UI launches immediately. If acting as a Host, click the **"Install Driver (UAC)"** button in the Display Engine card once to initialize the virtual display driver.

### Method 2: NSIS Setup Wizard (`.exe`)

1. **Download the Installer:**
   - Grab `Wireless-Monitor_0.1.0_x64-setup.exe` (or `Wireless-Monitor-Setup.exe`) from the [GitHub Releases](https://github.com/Ismail-Saihan/wireless-monitor/releases) page or build it locally using `.\scripts\build-windows.ps1`.
2. **Run the Setup Wizard:**
   - Double-click `Wireless-Monitor_0.1.0_x64-setup.exe`.
   - Choose your installation mode:
     - **Install for anyone who uses this computer (all users)** — Installs to `C:\Program Files\Wireless Monitor\` (requires UAC prompt).
     - **Install just for me** — Installs to `%LOCALAPPDATA%\Programs\Wireless Monitor\`.
   - Click **Install**.
3. **Shortcuts & Desktop Integration:**
   - The installer automatically creates a **Start Menu** entry with the official icon and an optional **Desktop** shortcut.
4. **Virtual Display Driver (Host Mode Only):**
   - If this PC will act as a **Host** (extending its screen to another device):
     - Open the app. In the **Display Engine** card, click the prominent **"Install Driver (UAC)"** button.
     - Accept the Windows User Account Control prompt. The companion service and signed driver certificate will be installed automatically.
     - *Note: If you are only using this computer as a Receiver (external monitor), no driver installation is required!*
5. **Uninstallation:**
   - Go to Windows **Settings** > **Installed Apps** (or Control Panel > Programs and Features).
   - Find **Wireless Monitor** and click **Uninstall**.
   - The uninstaller cleanly removes binaries, shortcuts, and driver registration.

### Method 3: MSI Enterprise Installer (`.msi`)

For system administrators deploying across workstations:
```cmd
:: Interactive install:
msiexec /i Wireless-Monitor_0.1.0_x64_en-US.msi

:: Unattended / Silent install:
msiexec /i Wireless-Monitor_0.1.0_x64_en-US.msi /quiet /qn /norestart

:: Silent uninstall:
msiexec /x Wireless-Monitor_0.1.0_x64_en-US.msi /quiet /qn
```

---

## 🐧 Linux Installation

We provide two standard Linux distribution formats:
1. **Debian Package (`.deb`)** — *For Ubuntu, Debian, Linux Mint, Pop!_OS.*
2. **Standalone AppImage (`.AppImage`)** — *For Fedora, Arch Linux, openSUSE, or portable use.*

### Method 1: Debian Package (`.deb`)

1. **Download the Package:**
   - Download `wireless-monitor_0.1.0_amd64.deb` from GitHub Releases.
2. **Install via APT:**
   ```bash
   sudo apt update
   sudo apt install ./wireless-monitor_0.1.0_amd64.deb
   ```
   *APT will automatically resolve and install required dependencies (`libpipewire-0.3-0`, `libdbus-1-3`, `libxdo3`, `libwebkit2gtk-4.1-0`).*
3. **Launch the Application:**
   - Search for **Wireless Monitor** in your GNOME Shell / KDE Plasma Application Launcher.
   - Or launch from terminal:
     ```bash
     wireless-monitor
     ```
4. **Uninstallation:**
   ```bash
   sudo apt remove wireless-monitor
   ```

### Method 2: Standalone AppImage (`.AppImage`)

1. **Download the AppImage:**
   - Download `Wireless-Monitor_0.1.0_amd64.AppImage`.
2. **Make Executable & Run:**
   ```bash
   chmod +x Wireless-Monitor_0.1.0_amd64.AppImage
   ./Wireless-Monitor_0.1.0_amd64.AppImage
   ```
3. *Optional Desktop Integration:* If you use `appimaged` or `AppImageLauncher`, opening the file will automatically register it into your desktop application menu.

---

## 🛠️ Building Installers From Source

If you want to build the turnkey installer packages yourself:

### On Windows (produces `.exe` and `.msi`):
```powershell
.\scripts\build-windows.ps1
```
*Generated installers are placed in `src-tauri\target\release\bundle\nsis\` and `bundle\msi\`.*

### On Linux (produces `.deb` and `.AppImage`):
```bash
./scripts/build-linux.sh
```
*Generated packages are placed in `src-tauri/target/release/bundle/deb/` and `bundle/appimage/`.*

### Automated GitHub Actions CI/CD:
Tagging any commit with `v*` (e.g. `git tag v0.1.0 && git push origin v0.1.0`) triggers `.github/workflows/release.yml`, which automatically builds all 4 installer packages on clean cloud runners and publishes them to GitHub Releases.
