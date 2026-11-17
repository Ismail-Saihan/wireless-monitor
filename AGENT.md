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
- **Networking & Streaming:** WebRTC (`webrtc-rs`), RFC 3550 & RFC 6184 RTP packetization, mDNS ZeroConf discovery (`mdns-sd`)
- **Video Rendering:** Hardware-accelerated presentation surface (`wgpu` / native borderless surface)

---

## 2. Current Implementation Status (Phase 1 MVP Complete)

Phase 1 (Windows Host → Linux Receiver baseline) is fully implemented, verified with 67/67 passing tests, and merged to `master`:

| Subsystem | Source Path | Key Traits / Structs | Test Suite |
| :--- | :--- | :--- | :--- |
| **Virtual Display Manager** | `src-tauri/src/display/` | `VirtualDisplayManager`, `DisplayConfig`, `WindowsIddCxManager` | `verify_display.py` |
| **Screen Capture & Cursor** | `src-tauri/src/capture/` | `ScreenCapturer`, `CapturedFrame`, `DxgiCapturer` | `verify_capture.py` |
| **Video Encoding & WebRTC** | `src-tauri/src/stream/` | `VideoEncoder`, `EncodedPacket`, `RtpPacketizer`, `WebRtcHostSession` | `verify_stream.py` |
| **mDNS & PIN Pairing** | `src-tauri/src/discovery/`, `src-tauri/src/pairing/` | `DiscoveryService`, `MdnsDiscovery`, `PinAuthManager`, `PairingSession` | `verify_discovery_pairing.py` |
| **Monitor Receiver** | `src-tauri/src/receiver/` | `MonitorReceiver`, `ClientScreenGeometry`, `PresentationSurface` | `verify_receiver.py` |
| **Orchestration & UI** | `src-tauri/src/lib.rs`, `ui/` | `AppState`, Tauri IPC handlers, `StatusHeader`, `DeviceList`, `PairingModal` | `verify_e2e.py` |

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
│       │   └── 2026-11-17-wireless-monitor-design.md
│       └── plans/
│           └── 2026-11-17-wireless-monitor-mvp.md
│
├── drivers/                              <-- Bundled virtual display drivers
│   └── windows/
│       ├── install_driver.bat            <-- Driver installation helper (elevated)
│       └── uninstall_driver.bat          <-- Clean driver teardown helper (elevated)
│
├── src-tauri/                            <-- Rust backend core
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── src/
│   │   ├── main.rs                       <-- Application entry point
│   │   ├── lib.rs                        <-- Tauri command routing, AppState & teardown hooks
│   │   ├── display/                      <-- Virtual display lifecycle management
│   │   │   ├── mod.rs
│   │   │   └── windows_iddcx.rs          <-- Windows IddCx driver control & dynamic monitor creation
│   │   ├── capture/                      <-- Frame capture from virtual monitor
│   │   │   ├── mod.rs
│   │   │   └── dxgi.rs                   <-- DXGI Desktop Duplication & cursor alpha compositing
│   │   ├── stream/                       <-- WebRTC & H.264 video pipeline
│   │   │   ├── mod.rs
│   │   │   ├── encoder.rs                <-- Hardware / software H.264 encoding abstraction
│   │   │   └── webrtc_session.rs         <-- WebRTC peer connection, RTP packetizer & RTCP PLI/FIR
│   │   ├── discovery/                    <-- Local network device discovery
│   │   │   ├── mod.rs
│   │   │   └── mdns.rs                   <-- mDNS announcement, browsing & RFC 1035 label sanitization
│   │   ├── pairing/                      <-- PIN authentication & security
│   │   │   ├── mod.rs
│   │   │   └── pin_auth.rs               <-- CSPRNG 4-digit PIN challenge & 3-attempt lockout
│   │   └── receiver/                     <-- Monitor mode presentation engine
│   │       ├── mod.rs
│   │       ├── webrtc_client.rs          <-- Client RTP stream consumer & signed sequence gap detection
│   │       └── surface.rs                <-- Hardware presentation surface (<1 frame latency queue)
│   └── tests/                            <-- Integration & unit tests
│       ├── capture_pipeline_test.rs
│       ├── discovery_pairing_test.rs
│       ├── display_manager_test.rs
│       ├── e2e_orchestration_test.rs
│       ├── receiver_test.rs
│       ├── webrtc_stream_test.rs
│       ├── verify_capture.py
│       ├── verify_discovery_pairing.py
│       ├── verify_display.py
│       ├── verify_e2e.py
│       ├── verify_receiver.py
│       └── verify_stream.py
│
└── ui/                                   <-- Frontend dashboard & pairing interface
    ├── index.html
    └── src/
        ├── main.ts
        ├── style.css
        └── components/
            ├── DeviceList.ts             <-- Discovered LAN peers list & action buttons
            ├── PairingModal.ts           <-- 4-digit PIN verification modal
            └── StatusHeader.ts           <-- Host/Monitor status pill, driver badge & metrics
```

---

## 4. Build, Test & Development Instructions

### Prerequisites
- **Node.js:** v18+ & `pnpm` (or `npm`)
- **Rust Toolchain:** `rustc` & `cargo` 1.75+ (`stable-x86_64-pc-windows-msvc`)
- **Python:** 3.10+ (for behavioral test verification suites)

### Running Tests & Verification
Run the verification test suite before committing any change:
```powershell
# Run full end-to-end integration test
python src-tauri/tests/verify_e2e.py

# Run all subsystem test suites
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
   - `DxgiCapturer` explicitly implements `unsafe impl Send` and `unsafe impl Sync` because all mutations require exclusive `&mut self` access. Do not remove these declarations.

2. **Never Hold `std::sync::MutexGuard` Across `.await` Points:**
   - Standard `std::sync::MutexGuard` is `!Send`. Holding a guard across an `.await` boundary makes the outer `Future` `!Send`, breaking Tokio and Tauri command handlers.
   - Always extract values outside the lock scope (e.g. `let session = self.host_session.lock().ok().and_then(|mut g| g.take());`) before calling `.await`.

3. **DXGI Desktop Duplication API Ordering:**
   - Always call `GetFramePointerShape()` *before* calling `ReleaseFrame()`. Calling `ReleaseFrame()` drops ownership of the desktop surface, causing subsequent `GetFramePointerShape()` calls to fail with `DXGI_ERROR_INVALID_CALL`.
   - When the mouse is stationary, DXGI reports `LastMouseUpdateTime == 0` and sets `PointerPosition.Visible` to `false`. Always retain and composite the last known cursor position so the mouse pointer does not vanish.
   - When `AccumulatedFrames == 0`, `ppDesktopResource` can be `None`. Handle this gracefully by releasing the frame lock and reusing the previous frame buffer.

4. **RTP 16-Bit Signed Sequence Wrapping:**
   - When calculating sequence deltas on incoming RTP packets, NEVER use unsigned wrapping subtraction.
   - Always calculate signed delta: `let diff = seq.wrapping_sub(expected) as i16;`.
   - If `diff > 0`: forward packet loss; increment loss count and request RTCP PLI/FIR.
   - If `diff <= 0`: late or duplicate packet; drop immediately without corrupting sequence tracking.

5. **Display Rollback & Teardown Safety:**
   - In `AppState::submit_pin()`, if screen capture fails to initialize, immediately roll back and destroy the created virtual display (`disp.destroy_display(monitor_id)`) to prevent orphaned displays.
   - `AppState::teardown()` and `WindowEvent::CloseRequested` / `Destroyed` ensure `destroy_all()` is executed on exit.

6. **mDNS Networking & DNS Label Rules:**
   - Always chain `.enable_addr_auto()` when creating `ServiceInfo` in `mdns-sd`.
   - Hostnames in DNS must be sanitized via `sanitize_dns_label()` to contain only lowercase alphanumeric characters and hyphens (RFC 1035 / RFC 6762).
   - Prioritize IPv4 addresses during peer address resolution before falling back to IPv6 link-local addresses.
   - Always call `daemon.unregister(&fullname)` when stopping the discovery service.

---

## 6. Phase 2 Roadmap & Swarm Workstreams

Phase 2 will complete bidirectional support and production driver bundling:

### Workstream 1: Linux Host Mode (Wayland)
- GNOME Mutter: Connect over D-Bus to `org.gnome.Mutter.RemoteDesktop.CreateVirtualMonitor`.
- KDE Plasma: Connect to KWin headless virtual output / D-Bus output interface.
- Screen Capture: Zero-copy PipeWire screencast stream connected to the H.264 video encoder.

### Workstream 2: Windows Receiver Mode
- Connect Windows Tauri receiver to Linux host WebRTC stream.
- Native presentation surface rendering using Direct3D11 / `wgpu` borderless fullscreen window.

### Workstream 3: Driver Packaging & Staging
- Download and bundle pre-compiled, WHQL-attestation-signed release binaries of `Virtual-Display-Driver` (`itsmikethetech/Virtual-Display-Driver`) into `drivers/windows/`.
- Verify one-click driver certificate import (`certutil`) and PnP device registration (`pnputil`).
