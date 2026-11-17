# AGENT.md - Wireless Virtual Monitor Project Guidelines

Welcome to the **Wireless Virtual Monitor** project repository. This guide provides comprehensive operational, architectural, and development rules for any AI agent or human developer working on this codebase.

---

## 1. Project Overview & Architecture

This application turns any device (Windows PC or Linux laptop) on the same local network into an authentic secondary wireless display for another device, recognized by the operating system as a native virtual monitor in system display settings.

### Core Philosophy
1. **True Virtual Display:** Never settle for a windowed screen-mirroring app. The OS must recognize the display as an authentic monitor (configurable resolution, scaling, and desktop extension).
2. **Sub-50ms Latency:** Low latency over local 5GHz Wi-Fi / Ethernet is essential for desktop productivity and fluid cursor movement.
3. **Safety & Zero Leftovers:** If the connection drops or the app exits, the virtual monitor must be cleanly unplugged immediately to avoid "trapping" the user's cursor on an invisible monitor.

### Key Technologies
- **Core Engine & Backend:** Rust (2021 edition)
- **Application Shell & UI:** Tauri 2.0 + TypeScript + Tailwind CSS
- **Virtual Display Driver (Windows):** Open-source signed Indirect Display Driver (IddCx / `Virtual-Display-Driver`)
- **Virtual Display Service (Linux):** GNOME Mutter RemoteDesktop D-Bus (`org.gnome.Mutter.RemoteDesktop`) & KDE Plasma D-Bus + PipeWire
- **Screen Capture:** Windows DXGI Desktop Duplication API / Linux PipeWire screencast
- **Networking & Streaming:** WebRTC (`webrtc-rs`), mDNS ZeroConf discovery (`mdns-sd`)
- **Video Rendering:** Hardware-accelerated presentation surface (`wgpu` / native sink)

---

## 2. Directory Structure

```
f:\Wireless Monitor Project\
├── AGENT.md                              <-- This operational guide
├── README.md                             <-- User-facing overview
├── package.json                          <-- Node/Tauri frontend dependencies
├── tsconfig.json                         <-- TypeScript configuration
├── vite.config.ts                        <-- Vite bundler configuration
│
├── docs/                                 <-- Architectural specifications and plans
│   └── superpowers/
│       ├── specs/
│       │   └── 2026-11-17-wireless-monitor-design.md
│       └── plans/
│           └── 2026-11-17-wireless-monitor-mvp.md
│
├── drivers/                              <-- Bundled virtual display drivers
│   └── windows/
│       ├── install_driver.bat            <-- Driver installation helper
│       └── uninstall_driver.bat          <-- Clean driver teardown helper
│
├── src-tauri/                            <-- Rust backend core
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── src/
│   │   ├── main.rs                       <-- Application entry point
│   │   ├── lib.rs                        <-- Tauri command routing & state
│   │   ├── display/                      <-- Virtual display lifecycle management
│   │   │   ├── mod.rs
│   │   │   └── windows_iddcx.rs          <-- Windows IddCx driver control
│   │   ├── capture/                      <-- Frame capture from virtual monitor
│   │   │   ├── mod.rs
│   │   │   └── dxgi.rs                   <-- DXGI Desktop Duplication (Windows)
│   │   ├── stream/                       <-- WebRTC & H.264 video pipeline
│   │   │   ├── mod.rs
│   │   │   ├── encoder.rs                <-- Hardware / software H.264 encoding
│   │   │   └── webrtc_session.rs         <-- WebRTC peer connection
│   │   ├── discovery/                    <-- Local network device discovery
│   │   │   ├── mod.rs
│   │   │   └── mdns.rs                   <-- mDNS announcement and listener
│   │   ├── pairing/                      <-- PIN authentication & security
│   │   │   ├── mod.rs
│   │   │   └── pin_auth.rs               <-- 4-digit PIN verification
│   │   └── receiver/                     <-- Monitor mode presentation engine
│   │       ├── mod.rs
│   │       ├── webrtc_client.rs          <-- Client RTP stream consumer
│   │       └── surface.rs                <-- Low-latency fullscreen renderer
│   └── tests/                            <-- Integration & unit tests
│
└── ui/                                   <-- Frontend dashboard & pairing interface
    ├── index.html
    └── src/
        ├── main.ts
        ├── style.css
        └── components/
            ├── DeviceList.ts
            ├── PairingModal.ts
            └── StatusHeader.ts
```

---

## 3. Build & Development Instructions

### Prerequisites
- **Node.js:** v18+ & `pnpm` (or `npm`)
- **Rust Toolchain:** `rustc` & `cargo` 1.75+ (`stable-x86_64-pc-windows-msvc`)
- **Visual Studio C++ Build Tools:** Required on Windows for MSVC linking.

### Development Commands
```bash
# Install frontend dependencies
pnpm install

# Run the Tauri app in development mode
pnpm tauri dev

# Run Rust unit and integration tests
cargo test --manifest-path src-tauri/Cargo.toml

# Build release binaries
pnpm tauri build
```

---

## 4. Coding & Architecture Rules for Agents

### 1. Test-Driven Development (TDD)
- When implementing any new module or fixing a bug, write the test first.
- Verify the test fails for the expected reason before implementing the functionality.
- Keep tests modular: use mocks/stubs for hardware capture and driver APIs when testing high-level state machines.

### 2. Driver & System Safety
- **Never leave orphan virtual displays:** Always ensure `VirtualDisplayManager::destroy_all()` is hooked into shutdown signals (`Ctrl+C`, application exit, panic hooks).
- **Never bypass UAC maliciously:** Driver installation requires Administrator privileges; trigger elevation through standard Windows prompts or explicit user scripts.

### 3. Latency & Memory Constraints
- Avoid unnecessary memory copies: Frame buffers from DXGI capture should move directly into the encoder surface without heap cloning.
- Drop late frames on network congestion: WebRTC jitter buffers must prioritize real-time freshness over playing stale frames.

### 4. Commits & Verification
- Commit frequently with concise, conventional commit messages (`feat: ...`, `fix: ...`, `test: ...`, `refactor: ...`).
- Verify all unit tests pass before marking any task complete.
