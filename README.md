# Wireless Virtual Monitor 🖥️📡

A lightweight, cross-platform bidirectional application built with **Rust + Tauri** that allows you to turn your Windows PC or Linux laptop into an authentic wireless secondary display over your local Wi-Fi or Ethernet.

Unlike typical remote desktop or windowed mirroring tools, this application creates a **true virtual monitor** recognized natively by your operating system's Display Settings (Windows Display Settings / GNOME & KDE Settings).

---

## Features
- **True Virtual Monitor:** Move windows, adjust resolution, rearrange monitor layout, and drag the mouse seamlessly across physical and wireless screens.
- **Bidirectional Support:** Extend Windows desktop to a Linux laptop, or extend Linux desktop to a Windows PC.
- **Ultra-Low Latency:** Optimized WebRTC streaming with GPU hardware encoding (NVENC, QuickSync, VA-API) targeting sub-50ms glass-to-glass latency.
- **LAN Auto-Discovery:** Automatically detects nearby devices on your network via mDNS.
- **PIN-Secured Pairing:** Secure 4-digit PIN authentication prevents unauthorized screen access.
- **Automated Windows Driver Setup:** One-click registration of the open-source WHQL-signed IddCx virtual display driver.

---

## Quick Start

### 1. Requirements
- Windows 10/11 (64-bit) or Linux (Wayland: GNOME or KDE)
- Node.js 18+ and `pnpm` (or `npm`)
- Rust toolchain (`cargo` 1.75+)

### 2. Run in Development
```bash
# Install UI dependencies
pnpm install

# Start Tauri development mode
pnpm tauri dev
```

---

## Documentation
- 📄 [Architecture Design Spec](docs/superpowers/specs/2026-11-17-wireless-monitor-design.md)
- 📋 [Implementation Plan](docs/superpowers/plans/2026-11-17-wireless-monitor-mvp.md)
- 🤖 [Agent Guidelines](AGENT.md)
