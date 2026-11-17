# Wireless Virtual Monitor 🖥️📡

A lightweight, cross-platform bidirectional application built with **Rust + Tauri 2.0** that allows you to turn your Windows PC or Linux laptop into an authentic wireless secondary display over local Wi-Fi or Ethernet.

Unlike typical remote desktop or windowed mirroring tools, this application creates a **true virtual monitor** recognized natively by your operating system's Display Settings (Windows Display Settings / GNOME & KDE Wayland Settings), with dynamic desktop extension, repositioning, and scaling.

---

## Key Features

- **True Virtual Monitor:** Move windows, adjust resolution, rearrange monitor layout, and drag the mouse seamlessly across physical and wireless screens.
- **Full Bidirectional Support:**
  - **Windows Host → Linux Receiver:** DXGI desktop capture and IddCx virtual monitor streaming to Linux presentation surface.
  - **Linux Host → Windows Receiver:** GNOME Mutter & FreeDesktop XDG Desktop Portal (KDE Plasma 6) D-Bus virtual display with PipeWire screencast capture streaming to Windows Direct3D11 borderless fullscreen surface.
- **Reverse Monitor Initiation ("Use as Monitor"):** Initiate a connection from either direction—click "Use as Monitor" on any discovered host device to instantly convert your local laptop/PC into that host's secondary screen.
- **Two-Stage Pairing & Approval Dialog:** Security-first connection handshake featuring an explicit receiver approval modal, CSPRNG 4-digit PIN verification, 3-attempt lockout protection, and 300-second session expiration.
- **Stream Settings & Dynamic Resolution Handshake:** Slide-out drawer with Auto resolution mode (automatically matches the receiver's physical screen geometry and refresh rate), presets for 1080p, 1440p, and 4K, or custom resolutions, with dynamic runtime encoder bitrate adjustment.
- **Ultra-Low Latency:** Optimized WebRTC streaming with RFC 3550 & RFC 6184 RTP packetization and a sub-frame (`<1 frame`) latency drop queue targeting **sub-50ms glass-to-glass latency**.
- **LAN Auto-Discovery:** Automatically detects nearby devices on your local network via mDNS (`_wireless-monitor._tcp.local`) enriched with capability TXT records.
- **Turnkey Production Packaging:**
  - **Windows:** NSIS & MSI installers bundling signed Indirect Display Driver (IddCx) binaries (`vdd.exe`, `.inf`, `.cer`, `.sys`, `.dll`) and automated UAC elevation scripts.
  - **Linux:** `.deb` and AppImage packages bundling system dependencies (`libpipewire-0.3-0`, `libdbus-1-3`, `libxdo3`).
- **Zero-Orphan Teardown Safety:** Automatically destroys virtual displays and tears down capture sessions on disconnect, window close, or crash.

---

## Architecture Overview

```
                   LOCAL NETWORK (mDNS Discovery & WebRTC)
 ┌────────────────────────────────────────────────────────────────────────┐
 │                                                                        │
 │  ┌─────────────────────────┐             ┌──────────────────────────┐  │
 │  │      HOST DEVICE        │             │      RECEIVER DEVICE     │  │
 │  │                         │             │                          │  │
 │  │ 1. Virtual Display Mgr  │             │ 1. Fullscreen Renderer   │  │
 │  │    (IddCx / Wayland)    │             │    (Direct3D11 / Native) │  │
 │  │            │            │             │            ▲             │  │
 │  │            ▼            │             │            │             │  │
 │  │ 2. Screen Capture       │   WebRTC    │ 2. H.264 Depacketizer    │  │
 │  │    (DXGI / PipeWire)    │  Stream     │    (<1 Frame Queue)      │  │
 │  │            │            │ ──────────> │            ▲             │  │
 │  │            ▼            │  (H.264)    │            │             │  │
 │  │ 3. Video Encoder        │             │ 3. WebRTC PeerClient     │  │
 │  │    (Hardware / Software)│             │    (webrtc-rs)           │  │
 │  │                         │ <────────── │                          │  │
 │  │ 4. mDNS (Advertiser     │  Handshake  │ 4. mDNS (Advertiser      │  │
 │  │    & Browser) + PIN     │ & Res Sync  │    & Browser) + PIN      │  │
 │  └─────────────────────────┘             └──────────────────────────┘  │
 └────────────────────────────────────────────────────────────────────────┘
```

---

## Quick Start

### 1. Prerequisites
- **Operating System:** Windows 10/11 (64-bit) or Linux (Wayland: GNOME 40+ or KDE Plasma 6+)
- **Node.js:** v18+ & `pnpm` (or `npm`)
- **Rust Toolchain:** `rustc` & `cargo` 1.75+ (`stable-x86_64-pc-windows-msvc`)
- **Python:** 3.10+ (for verification suites)

### 2. Driver Setup (Windows Host Mode)
For Windows Host mode, install the staged virtual display driver (requires Administrator privileges):
```powershell
# From an elevated PowerShell prompt:
.\drivers\windows\install_driver.bat
```
To cleanly uninstall:
```powershell
.\drivers\windows\uninstall_driver.bat
```

### 3. Run in Development
```bash
# Install frontend dependencies
pnpm install

# Start Tauri development application
pnpm tauri dev
```

---

## Testing & Verification

Comprehensive test suites cover unit contracts, integration, packaging, and full end-to-end bidirectional streaming:

```powershell
# Run full Phase 3 comprehensive integration verification suite
python src-tauri/tests/verify_phase3.py

# Run Phase 3 component verification suites:
python src-tauri/tests/verify_discovery_caps.py      # mDNS capability TXT records & reverse initiation
python src-tauri/tests/verify_pairing_security.py    # Approval workflow, queue, lockout & TTL
python src-tauri/tests/verify_settings_handshake.py  # StreamSettings & dynamic resolution negotiation
python src-tauri/tests/verify_packaging.py           # NSIS/MSI driver bundling & Linux packaging

# Run full Phase 2 bidirectional end-to-end integration test
python src-tauri/tests/verify_phase2.py

# Run Phase 1 end-to-end regression test
python src-tauri/tests/verify_e2e.py

# Component-level verification suites:
python src-tauri/tests/verify_linux_display.py       # Linux Wayland D-Bus manager (Mutter & Portal)
python src-tauri/tests/verify_pipewire_capture.py    # PipeWire capture & cursor compositing
python src-tauri/tests/verify_windows_receiver.py    # Windows Direct3D11 presentation surface
python src-tauri/tests/verify_driver_staging.py      # Windows driver packaging & PE validation
python src-tauri/tests/verify_display.py             # Windows IddCx display manager
python src-tauri/tests/verify_capture.py             # Windows DXGI desktop duplication
python src-tauri/tests/verify_stream.py              # WebRTC & H.264 streaming pipeline
python src-tauri/tests/verify_discovery_pairing.py   # mDNS discovery & PIN pairing
python src-tauri/tests/verify_receiver.py            # Receiver presentation & RTP depacketizer

# Frontend TypeScript type check
npx tsc --noEmit
```

---

## Documentation

- 📄 [Phase 1 MVP Design Spec](docs/superpowers/specs/2026-11-17-wireless-monitor-design.md)
- 📄 [Phase 2 Bidirectional Design Spec](docs/superpowers/specs/2026-11-17-phase2-bidirectional-design.md)
- 📄 [Phase 3 Hardening Design Spec](docs/superpowers/specs/2026-11-17-phase3-hardening-design.md)
- 📋 [Phase 1 Implementation Plan](docs/superpowers/plans/2026-11-17-wireless-monitor-mvp.md)
- 📋 [Phase 2 Implementation Plan](docs/superpowers/plans/2026-11-17-wireless-monitor-phase2.md)
- 📋 [Phase 3 Implementation Plan](docs/superpowers/plans/2026-11-17-wireless-monitor-phase3.md)
- 🤖 [Operational & Architecture Guidelines](AGENT.md)
