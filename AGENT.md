# AGENT.md - Wireless Virtual Monitor Project Guidelines

Welcome to the **Wireless Virtual Monitor** project repository. This guide provides comprehensive operational, architectural, and development rules for any AI agent or human developer working on this codebase.

---

## 1. Project Overview & Architecture

This application turns any device (Windows PC or Linux laptop) on the same local network into an authentic secondary wireless display for another device, recognized by the operating system as a native virtual monitor in system display settings.

### Core Philosophy
1. **True Virtual Display:** Never settle for a windowed screen-mirroring app. The OS must recognize the display as an authentic monitor (configurable resolution, scaling, and desktop extension).
2. **Sub-50ms Latency:** Low latency over local 5GHz Wi-Fi / Ethernet is essential for desktop productivity and fluid cursor movement.
3. **Safety & Zero Leftovers:** If the connection drops or the app exits, the virtual monitor must be cleanly unplugged immediately to avoid "trapping" the user's cursor on an invisible monitor.
4. **Bidirectional Orchestration:** Any device can seamlessly function as either a **Host** (originating the virtual display and encoding video) or a **Receiver** (presenting the display borderless fullscreen).

### Key Technologies
- **Core Engine & Backend:** Rust (2021 edition)
- **Application Shell & UI:** Tauri 2.0 + TypeScript + Tailwind CSS
- **Virtual Display Driver (Windows):** Open-source signed Indirect Display Driver (IddCx / `Virtual-Display-Driver`) staged with companion IPC service (`vdd.exe`)
- **Virtual Display Service (Linux):** GNOME Mutter RemoteDesktop D-Bus (`org.gnome.Mutter.RemoteDesktop`) & FreeDesktop XDG Desktop Portal (`org.freedesktop.portal.RemoteDesktop` & `ScreenCast`)
- **Screen Capture:** Windows DXGI Desktop Duplication API / Linux PipeWire screencast stream with cursor compositing
- **Networking & Streaming:** WebRTC (`webrtc-rs`), RFC 3550 & RFC 6184 RTP packetization, mDNS ZeroConf discovery (`mdns-sd`)
- **Video Presentation:** Windows Direct3D11 swap chain presentation surface (`DXGI_FORMAT_B8G8R8A8_UNORM`) & Linux native surface

---

## 2. Current Implementation Status (Phase 3 Complete: Production Packaging, Security, UX & Capabilities)

Phase 1 (Windows Host → Linux Receiver baseline), Phase 2 (Linux Host, Windows Receiver, Driver Staging, and Bidirectional Orchestration), and Phase 3 (Distribution Packaging, Connection Approval Security, Stream Settings UX, and Capabilities) are fully implemented and verified:

| Subsystem | Source Path | Key Traits / Structs | Test Suite |
| :--- | :--- | :--- | :--- |
| **Turnkey Production Installers** | `src-tauri/icons/`, `scripts/` | NSIS (.exe), MSI (.msi), DEB (.deb), AppImage, multi-res icons | `verify_installers.py` |
| **Comprehensive Phase 3 Integration Suite** | `src-tauri/tests/verify_phase3.py` | Full integration & behavioral verification across all Phase 3 interfaces | `verify_phase3.py` |
| **mDNS Capabilities & Reverse Initiation** | `src-tauri/src/discovery/` | `PeerDevice` (capabilities, max_res, fps), `initiate_use_as_monitor` | `verify_discovery_caps.py` |
| **Connection Approval & Enhanced Security** | `src-tauri/src/pairing/pin_auth.rs` | `IncomingRequest`, `PairingState::PendingApproval`, 3-attempt lockout, 300s TTL | `verify_pairing_security.py` |
| **Stream Settings & Dynamic Resolution** | `src-tauri/src/stream/settings.rs` | `StreamSettings`, `resolve_display_config`, dynamic bitrate adjustment | `verify_settings_handshake.py` |
| **Distribution Packaging & Bundling** | `src-tauri/tauri.conf.json` | NSIS & MSI (Windows + drivers), DEB & AppImage (Linux + PipeWire/D-Bus) | `verify_packaging.py` |
| **Approval Modal & Settings Drawer UI** | `ui/src/components/` | `ApprovalModal.ts`, `SettingsDrawer.ts`, capabilities badges, role actions | `npx tsc --noEmit` |
| **Linux Wayland Display Manager** | `src-tauri/src/display/linux_wayland.rs` | `WaylandDisplayManager`, `WaylandBackend` (Mutter/Portal/Simulated) | `verify_linux_display.py` |
| **Linux PipeWire Screen Capturer** | `src-tauri/src/capture/pipewire.rs` | `PipeWireCapturer`, `PipeWireCursorMetadata`, `composite_pipewire_cursor` | `verify_pipewire_capture.py` |
| **Windows D3D11 Presentation Surface** | `src-tauri/src/receiver/windows_surface.rs` | `WindowsD3D11Surface`, dynamic 2D texture, <1 frame latency dropping | `verify_windows_receiver.py` |
| **Windows Driver Packaging & Staging** | `drivers/windows/` | `VirtualDisplayDriver.inf`, `.cer`, `vdd.exe`, x64 `.sys`/`.dll`, batch scripts | `verify_driver_staging.py` |
| **Windows IddCx Display Manager** | `src-tauri/src/display/windows_iddcx.rs` | `WindowsIddCxManager`, `DisplayConfig` | `verify_display.py` |
| **Windows DXGI Capturer & Cursor** | `src-tauri/src/capture/dxgi.rs` | `DxgiCapturer`, `composite_cursor` | `verify_capture.py` |
| **Video Encoding & WebRTC** | `src-tauri/src/stream/` | `VideoEncoder`, `EncodedPacket`, `RtpPacketizer`, `WebRtcHostSession` | `verify_stream.py` |
| **mDNS & PIN Pairing** | `src-tauri/src/discovery/`, `src-tauri/src/pairing/` | `DiscoveryService`, `MdnsDiscovery`, `PinAuthManager`, `PairingSession` | `verify_discovery_pairing.py` |
| **Monitor Receiver** | `src-tauri/src/receiver/` | `MonitorReceiver`, `ClientScreenGeometry`, `PresentationSurface` | `verify_receiver.py` |
| **Phase 2 Bidirectional Orchestration** | `src-tauri/src/lib.rs`, `ui/` | `AppState`, `Phase2Status`, bidirectional Tauri IPC commands, UI Dashboard | `verify_phase2.py`, `verify_e2e.py` |

---

## 3. Directory Structure

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
│       │   ├── 2026-11-17-wireless-monitor-design.md
│       │   ├── 2026-11-17-phase2-bidirectional-design.md
│       │   └── 2026-11-17-phase3-security-settings-packaging-design.md
│       └── plans/
│           ├── 2026-11-17-wireless-monitor-mvp.md
│           ├── 2026-11-17-wireless-monitor-phase2.md
│           └── 2026-11-17-wireless-monitor-phase3.md
│
├── drivers/                              <-- Bundled virtual display drivers
│   └── windows/
│       ├── VirtualDisplayDriver.cer      <-- Code signing security certificate
│       ├── VirtualDisplayDriver.inf      <-- IddCx driver manifest
│       ├── vdd.exe                       <-- 64-bit companion IPC service binary
│       ├── install_driver.bat            <-- Turnkey elevated installation automation script
│       ├── uninstall_driver.bat          <-- Clean driver teardown automation script
│       └── x64/
│           ├── VirtualDisplayDriver.sys  <-- KMDF kernel-mode driver
│           └── VirtualDisplayDriver.dll  <-- UMDF user-mode display driver
│
├── src-tauri/                            <-- Rust backend core
│   ├── Cargo.toml
│   ├── tauri.conf.json                   <-- NSIS, MSI, deb, appimage bundle targets & driver mappings
│   ├── src/
│   │   ├── main.rs                       <-- Application entry point
│   │   ├── lib.rs                        <-- Tauri command routing, AppState & teardown hooks
│   │   ├── display/                      <-- Virtual display lifecycle management
│   │   │   ├── mod.rs
│   │   │   ├── linux_wayland.rs          <-- GNOME Mutter & XDG Desktop Portal D-Bus driver
│   │   │   └── windows_iddcx.rs          <-- Windows IddCx driver control & dynamic monitor creation
│   │   ├── capture/                      <-- Frame capture from virtual monitor
│   │   │   ├── mod.rs
│   │   │   ├── dxgi.rs                   <-- DXGI Desktop Duplication & cursor alpha compositing
│   │   │   └── pipewire.rs               <-- PipeWire screencast capture & cursor compositing
│   │   ├── stream/                       <-- WebRTC & H.264 video pipeline
│   │   │   ├── mod.rs
│   │   │   ├── settings.rs               <-- StreamSettings (Auto/1080p/1440p/4k/Custom) & dynamic negotiation
│   │   │   ├── encoder.rs                <-- Hardware / software H.264 encoding & dynamic bitrate update
│   │   │   └── webrtc_session.rs         <-- WebRTC peer connection, RTP packetizer & RTCP PLI/FIR
│   │   ├── discovery/                    <-- Local network device discovery
│   │   │   ├── mod.rs
│   │   │   └── mdns.rs                   <-- mDNS announcement with capability TXT records & browsing
│   │   ├── pairing/                      <-- PIN authentication & connection approval
│   │   │   ├── mod.rs
│   │   │   └── pin_auth.rs               <-- IncomingRequest queue, approval workflow, 3-attempt lockout, 300s TTL
│   │   └── receiver/                     <-- Monitor mode presentation engine
│   │       ├── mod.rs
│   │       ├── webrtc_client.rs          <-- Client RTP stream consumer & signed sequence gap detection
│   │       ├── surface.rs                <-- Hardware presentation surface (<1 frame latency queue)
│   │       └── windows_surface.rs        <-- Direct3D11 swap chain presentation surface
│   └── tests/                            <-- Integration & unit tests
│       ├── capture_pipeline_test.rs
│       ├── discovery_caps_test.rs
│       ├── discovery_pairing_test.rs
│       ├── display_manager_test.rs
│       ├── driver_staging_test.rs
│       ├── e2e_bidirectional_test.rs
│       ├── e2e_orchestration_test.rs
│       ├── linux_display_test.rs
│       ├── packaging_test.rs
│       ├── pairing_security_test.rs
│       ├── pipewire_capture_test.rs
│       ├── receiver_test.rs
│       ├── settings_handshake_test.rs
│       ├── webrtc_stream_test.rs
│       ├── windows_receiver_test.rs
│       ├── verify_capture.py
│       ├── verify_discovery_caps.py
│       ├── verify_discovery_pairing.py
│       ├── verify_display.py
│       ├── verify_driver_staging.py
│       ├── verify_e2e.py
│       ├── verify_linux_display.py
│       ├── verify_packaging.py
│       ├── verify_pairing_security.py
│       ├── verify_phase2.py
│       ├── verify_phase3.py
│       ├── verify_pipewire_capture.py
│       ├── verify_receiver.py
│       ├── verify_settings_handshake.py
│       ├── verify_stream.py
│       └── verify_windows_receiver.py
│
└── ui/                                   <-- Frontend dashboard & pairing interface
    ├── index.html
    └── src/
        ├── main.ts                       <-- App initialization, polling, role switching, modal wiring
        ├── style.css
        └── components/
            ├── ApprovalModal.ts          <-- Incoming connection request approval & PIN challenge modal
            ├── DeviceList.ts             <-- Discovered LAN peers list, capability pills & role action buttons
            ├── PairingModal.ts           <-- 4-digit PIN verification modal
            ├── SettingsDrawer.ts         <-- Slide-out stream settings drawer (resolution, FPS, bitrate)
            └── StatusHeader.ts           <-- Host/Receiver role switcher, OS badge & settings trigger
```

---

## 4. Build, Test & Development Instructions

### Prerequisites
- **Node.js:** v18+ & `pnpm` (or `npm`)
- **Rust Toolchain:** `rustc` & `cargo` 1.75+ (`stable-x86_64-pc-windows-msvc`)
- **Python:** 3.10+ (for behavioral test verification suites)

### Running Tests & Verification
Run the verification test suites before committing any change:
```powershell
# Run full Phase 3 comprehensive integration verification suite
python src-tauri/tests/verify_phase3.py

# Run Phase 3 component verification suites
python src-tauri/tests/verify_discovery_caps.py
python src-tauri/tests/verify_pairing_security.py
python src-tauri/tests/verify_settings_handshake.py
python src-tauri/tests/verify_packaging.py

# Run full Phase 2 bidirectional end-to-end integration test
python src-tauri/tests/verify_phase2.py

# Run Phase 1 end-to-end integration test
python src-tauri/tests/verify_e2e.py

# Run Phase 2 component verification suites
python src-tauri/tests/verify_linux_display.py
python src-tauri/tests/verify_pipewire_capture.py
python src-tauri/tests/verify_windows_receiver.py
python src-tauri/tests/verify_driver_staging.py

# Run Phase 1 subsystem test suites
python src-tauri/tests/verify_stream.py
python src-tauri/tests/verify_capture.py
python src-tauri/tests/verify_display.py
python src-tauri/tests/verify_discovery_pairing.py
python src-tauri/tests/verify_receiver.py

# Type-check frontend TypeScript
npx tsc --noEmit
```

### Development Commands
```bash
# Install frontend dependencies
pnpm install

# Run the Tauri app in development mode
pnpm tauri dev

# Run Rust unit and integration tests
cargo test --manifest-path src-tauri/Cargo.toml
```

---

## 5. Architectural Invariants & Critical Lessons Learned

Any agent modifying this codebase MUST respect these established patterns:

1. **COM Pointers & Thread Safety (`Send + Sync`):**
   - In `windows` crate 0.58, COM interface pointers (`ID3D11Device`, `IDXGIOutputDuplication`) are `!Send` and `!Sync`.
   - `DxgiCapturer` and `WindowsD3D11Surface` explicitly implement `unsafe impl Send` and `unsafe impl Sync` because all mutations require exclusive `&mut self` access. Do not remove these declarations.

2. **Never Hold `std::sync::MutexGuard` Across `.await` Points:**
   - Standard `std::sync::MutexGuard` is `!Send`. Holding a guard across an `.await` boundary makes the outer `Future` `!Send`, breaking Tokio and Tauri command handlers.
   - Always extract values outside the lock scope (e.g. `let session = self.host_session.lock().ok().and_then(|mut g| g.take());`) before calling `.await`.

3. **DXGI Desktop Duplication API Ordering:**
   - Always call `GetFramePointerShape()` *before* calling `ReleaseFrame()`. Calling `ReleaseFrame()` drops ownership of the desktop surface, causing subsequent `GetFramePointerShape()` calls to fail with `DXGI_ERROR_INVALID_CALL`.
   - When the mouse is stationary, DXGI reports `LastMouseUpdateTime == 0` and sets `PointerPosition.Visible` to `false`. Always retain and composite the last known cursor position so the mouse pointer does not vanish.
   - When `AccumulatedFrames == 0`, `ppDesktopResource` can be `None`. Handle this gracefully by releasing the frame lock and reusing the previous frame buffer.

4. **Linux Wayland Virtual Output & D-Bus Lifecycle:**
   - GNOME Mutter: invoke `org.gnome.Mutter.RemoteDesktop.CreateVirtualMonitor` on session handle.
   - FreeDesktop XDG Portal: invoke `org.freedesktop.portal.RemoteDesktop.CreateSession` and `org.freedesktop.portal.ScreenCast.CreateSession`.
   - Do not call nested `tokio::runtime::Runtime::new()` within an already active Tokio thread pool; probe `tokio::runtime::Handle::try_current()` and block in place.

5. **PipeWire Cursor Compositing & Stride Alignment:**
   - Stride must be 16-byte aligned and satisfy `stride >= width * 4`.
   - In `Metadata` cursor mode, extract cursor position, clip boundary edges, and alpha blend onto the RGBA frame. In `Embedded` mode, the compositor has already rendered the cursor.

6. **Direct3D11 Presentation Surface & Sub-Frame Latency:**
   - Presentation queue depth must not exceed 2 frames. Drop the oldest unrendered frame when the queue overflows to prevent presentation bloat and enforce sub-50ms glass-to-glass latency.
   - Use `DXGI_FORMAT_B8G8R8A8_UNORM` dynamic textures with `UpdateSubresource` and `Present(0, 0)` for immediate non-blocking swap chain flips.

7. **Display Rollback & Teardown Safety:**
   - In `AppState::submit_pin()`, if screen capture fails to initialize, immediately roll back and destroy the created virtual display (`disp.destroy_display(monitor_id)`) to prevent orphaned displays.
   - `AppState::teardown()` and `WindowEvent::CloseRequested` / `Destroyed` ensure `destroy_all()` and presentation surface closing are executed on exit.

8. **Macroblock Alignment (Even Dimensions Required):**
   - H.264 video compression architectures (NVENC, AMF, QuickSync, OpenH264) mandate that frame width and height are even integers (divisible by 2) to align with chroma subsampling and macroblock structures.
   - All client screen geometries and custom stream settings must enforce `width % 2 == 0` and `height % 2 == 0`. Auto-resolution resolution rounds down odd client dimensions to the nearest even integer (`(w // 2) * 2`).

9. **Two-Stage Pairing Security (Approval Preceding PIN Challenge):**
   - Connection attempts generate an `IncomingRequest` in the `PairingState::PendingApproval` state.
   - PIN verification is strictly prohibited while pending approval. Only explicit receiver user authorization (`respond_to_request(peer_id, true)`) transitions the session to `PendingPin`.
   - Any rejection immediately moves the state to `Rejected` and purges the request from the pending queue.
   - Sessions are protected by a strict 3-attempt lockout and 300-second TTL to prevent brute-force exploitation.

10. **Dynamic Stream Settings & Display Negotiation:**
    - Stream settings decouple presentation configuration from device hardware via `StreamSettings` (`auto`, `1080p`, `1440p`, `4k`, `custom`).
    - In `auto` mode, the host automatically creates a virtual display matching the receiver's physical display geometry and refresh rate.
    - When stream settings are updated during runtime, the video encoder's target bitrate is dynamically adjusted without tearing down active WebRTC streaming sessions.

---

## 6. Phase 3 Completion Summary

All workstreams in Phase 3 (Production Packaging, Security Hardening, UX Refinements & Capabilities) are fully delivered and verified:
- **mDNS Capability Broadcast & Reverse Initiation:** Extended ZeroConf discovery with capability TXT records (`caps`, `res`, `fps`) with legacy fallback, and added `initiate_use_as_monitor` enabling reverse monitor initiation from the UI.
- **Connection Approval & Enhanced PIN Security:** Implemented two-stage connection approval workflow (`IncomingRequest`, `PairingState::PendingApproval`), 3-attempt lockout enforcement, 300s TTL session expiration, and zero display orphan guarantee.
- **Stream Settings & Dynamic Resolution Handshake:** Implemented `StreamSettings` subsystem (`auto`, `1080p`, `1440p`, `4k`, `custom`), dynamic geometry negotiation with receiver client, framerate (1-360) and bitrate (1-100 Mbps) bounds validation, and dynamic encoder bitrate adjustment.
- **Distribution Packaging & Bundling:** Configured `tauri.conf.json` for production distribution: Windows NSIS & MSI installers with staged virtual display driver resources (`drivers/windows/**/*`), Linux `.deb` and AppImage bundles with system dependencies (`libpipewire-0.3-0`, `libdbus-1-3`, `libxdo3`), and application metadata.
- **Frontend UX Refinements:** Built slide-out `SettingsDrawer`, `ApprovalModal` for incoming connections, capability badges in `DeviceList`, "Use as Monitor" action buttons, and background polling.
- **Comprehensive Verification Suite:** Delivered `src-tauri/tests/verify_phase3.py` executing all 7 Phase 3 behavioral contracts alongside syntax, interface, and driver asset checks. Zero regressions across Phase 1 and Phase 2.

---

## 7. Modern Minimal UI Architecture & Frontend Build Pipeline

The application frontend is built with **Vite + Tailwind CSS v3 + TypeScript**, engineered for desktop minimalism, fast render cycles, and low IPC overhead:

### 1. Build Pipeline & Style Ingestion
- `postcss.config.js` and `tailwind.config.js` configured in both project root and `ui/`, scanning `./ui/index.html` and `./ui/src/**/*.{js,ts,jsx,tsx}`.
- Style entrypoint `ui/src/style.css` imported at the top of `ui/src/main.ts` (`import "./style.css";`), ensuring production bundles compile >25 kB of CSS utilities.
- Glassmorphic panels (`.glass-panel`, `.glass-panel-subtle`) leverage `backdrop-blur-xl`, `bg-slate-900/80`, and translucent borders (`border-white/[0.08]`).

### 2. Real-Time Telemetry & Active Session HUD
- Dynamic Active Session HUD (`#active-session-card`) monitors stream state. When active, it displays connected peer hostname, negotiated resolution, and real-time glass-to-glass latency (`< 16 ms`).
- Role Switcher segmented control (`#btn-role-host` vs `#btn-role-receiver`) enables dynamic switching between transmitting a virtual display or acting as a secondary monitor receiver.

### 3. Device Discovery & Animated Radar
- When zero peers are visible on the local network, the UI renders an animated CSS radar scanner (`@keyframes radar-pulse`) indicating active mDNS browsing on `_wireless-monitor._tcp`.
- Discovered devices render capability badges (`1080p`, `1440p`, `4K`, `60 FPS`, `120 FPS`) and dual action gradient buttons (**Extend Screen** and **Use as Monitor**).

### 4. Low-Latency Stream Configuration Drawer
- Slide-out drawer (`SettingsDrawer`) manages `StreamSettings` persisted in `localStorage` and synchronized with the Rust backend via `invoke("set_stream_settings")`.
- Supports `auto` (matches physical client geometry), standard presets (1080p, 1440p, 4K), or custom pixel dimensions with even-dimension macroblock validation (`w % 2 == 0`).

---

## 8. Turnkey Production Packaging & CI/CD Release Automation

### 1. GitHub Remote Repository & Releases
- **Repository URL:** [https://github.com/Ismail-Saihan/wireless-monitor](https://github.com/Ismail-Saihan/wireless-monitor)
- **Releases Page:** [https://github.com/Ismail-Saihan/wireless-monitor/releases](https://github.com/Ismail-Saihan/wireless-monitor/releases)

### 2. Pre-Built Root Executables
- Standalone Portable Executable: `wireless-monitor.exe` (~9.67 MB)
- Turnkey NSIS Setup Wizard: `Wireless-Monitor-Setup.exe` (~2.20 MB)

### 3. Automated Cross-Platform CI/CD
- Workflow file: `.github/workflows/release.yml`
- Triggered automatically on push of version tags (`v*`) or manual `workflow_dispatch`.
- Builds in parallel across `windows-latest` (NSIS `.exe` + MSI `.msi`) and `ubuntu-22.04` (Debian `.deb` + AppImage) and publishes downloadable release assets.
