# Wireless Virtual Monitor (Phase 1 MVP) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a cross-platform, bidirectional wireless monitor application (Rust + Tauri) that allows extending the desktop onto a secondary device over LAN as a native virtual monitor detected by OS Display Settings, starting with the Windows Host → Linux Receiver MVP.

**Architecture:** A unified Tauri application running on both machines. On Windows Host, an open-source signed IddCx driver is automatically managed to plug in virtual displays matching client specs, frames are captured via DXGI Desktop Duplication, encoded to H.264, and streamed over WebRTC. On Linux Receiver, incoming WebRTC video is rendered in a hardware-accelerated borderless fullscreen presentation window.

**Tech Stack:** Rust 2021, Tauri 2.0, `windows` crate (DXGI/D3D11), `webrtc` crate, `mdns-sd`, `wgpu` / native video sink, TypeScript + Tailwind CSS for UI.

**Spec:** [`docs/superpowers/specs/2026-11-17-wireless-monitor-design.md`](file:///f:/Wireless%20Monitor%20Project/docs/superpowers/specs/2026-11-17-wireless-monitor-design.md)

---

## Global Constraints
- Target latency: Sub-50ms glass-to-glass latency over local 5GHz Wi-Fi / Ethernet.
- Host virtual monitor detection: Must appear in Windows Display Settings as an active, configurable monitor.
- Mouse traversal: Host mouse cursor must be captured and rendered directly in the video stream.
- Zero-config discovery: LAN discovery must operate without manual IP configuration via mDNS.
- Security: Must require a 4-digit PIN or explicit approval prompt before establishing display streaming.
- Host scope (v1): Windows (DXGI + IddCx) and Linux Wayland (GNOME/KDE + PipeWire). X11 host mode is out of scope.

## Review Focus
1. **Driver absent or uninstalled on Windows Host:** App must display clear elevation prompt to register the driver rather than crashing or hanging on device enumeration.
2. **Wi-Fi packet drop / jitter bursts:** WebRTC stream must drop late frames and trigger instantaneous IDR (keyframe) refresh instead of stalling or causing green/corrupted screen artifacts.
3. **Display resolution mismatch:** Receiver must report actual physical canvas pixel bounds so the virtual monitor aspect ratio matches client screen with no letterboxing or distortion.
4. **Sudden disconnect / crash of Receiver:** Host must gracefully detect lost WebRTC heartbeat and automatically unplug/destroy the virtual monitor within 3 seconds to avoid "trapping" the user's cursor on an invisible screen.
5. **Firewall / Port blockage:** mDNS or WebRTC UDP port blocking must provide actionable troubleshooting diagnostics in the UI rather than infinite loading spinner.

---

### Task 1: Repository Structure, Scaffolding & Configuration

**Files:**
- Create: `package.json`
- Create: `src-tauri/Cargo.toml`
- Create: `src-tauri/tauri.conf.json`
- Create: `src-tauri/src/main.rs`
- Create: `src-tauri/src/lib.rs`
- Create: `.gitignore`
- Create: `ui/index.html`
- Create: `ui/src/main.ts`
- Create: `ui/src/style.css`

**Interfaces:**
- Produces: Initial compilable Tauri 2.0 multi-crate project with frontend scaffold and backend placeholder commands.

- [ ] **Step 1: Write `.gitignore` and `package.json`**
  Set up Node/Tauri frontend tooling dependencies (TypeScript, Vite, Tailwind CSS).

- [ ] **Step 2: Write `src-tauri/Cargo.toml`**
  Declare dependencies: `tauri`, `serde`, `serde_json`, `tokio`, `tracing`, `tracing-subscriber`, `anyhow`, `thiserror`.

- [ ] **Step 3: Implement minimal Tauri app entry point in `src-tauri/src/main.rs` and `lib.rs`**
  Expose healthcheck IPC command `check_system_status() -> SystemStatus`.

- [ ] **Step 4: Create UI scaffold in `ui/`**
  Minimal responsive dashboard showing: "Mode: Host / Monitor", "Status: Idle", and "Available Peers".

- [ ] **Step 5: Verify build configuration**
  Validate configuration schemas and file paths.

---

### Task 2: Windows Virtual Display Driver (IddCx) Management Bridge

**Files:**
- Create: `src-tauri/src/display/mod.rs`
- Create: `src-tauri/src/display/windows_iddcx.rs`
- Create: `drivers/windows/install_driver.bat`
- Create: `drivers/windows/uninstall_driver.bat`
- Test: `src-tauri/tests/display_manager_test.rs`

**Interfaces:**
- Produces:
  ```rust
  pub struct DisplayConfig {
      pub width: u32,
      pub height: u32,
      pub refresh_rate: u32,
  }

  pub trait VirtualDisplayManager: Send + Sync {
      fn is_driver_installed(&self) -> Result<bool>;
      fn install_driver(&self) -> Result<()>;
      fn create_display(&mut self, config: &DisplayConfig) -> Result<u32>; // returns monitor id
      fn destroy_display(&mut self, monitor_id: u32) -> Result<()>;
      fn destroy_all(&mut self) -> Result<()>;
  }
  ```

- [ ] **Step 1: Write the failing unit tests in `src-tauri/tests/display_manager_test.rs`**
  Test display creation configuration validation, monitor ID tracking, and state cleanup.

- [ ] **Step 2: Implement `VirtualDisplayManager` for Windows using IddCx control interface in `src-tauri/src/display/windows_iddcx.rs`**
  Bridge to `Virtual-Display-Driver` IPC / registry / named pipe configuration to dynamically plug in and unplug display adapters.

- [ ] **Step 3: Implement driver installation detection and batch execution script**
  Detect whether the IddCx device is present in Windows device registry; provide elevated trigger to execute `devcon` / certificate import.

- [ ] **Step 4: Run unit tests and verify they pass**
  Run `cargo test --test display_manager_test` (or mock on non-Windows).

---

### Task 3: DXGI Desktop Duplication Screen Capture Pipeline (Windows)

**Files:**
- Create: `src-tauri/src/capture/mod.rs`
- Create: `src-tauri/src/capture/dxgi.rs`
- Test: `src-tauri/tests/capture_pipeline_test.rs`

**Interfaces:**
- Consumes: Monitor identifier or adapter output index from Task 2.
- Produces:
  ```rust
  pub struct CapturedFrame {
      pub width: u32,
      pub height: u32,
      pub stride: u32,
      pub data: Vec<u8>, // RGBA or NV12
      pub timestamp_us: u64,
  }

  pub trait ScreenCapturer: Send + Sync {
      fn start(&mut self, output_index: u32) -> Result<()>;
      fn capture_frame(&mut self, timeout_ms: u32) -> Result<Option<CapturedFrame>>;
      fn stop(&mut self) -> Result<()>;
  }
  ```

- [ ] **Step 1: Write unit tests in `src-tauri/tests/capture_pipeline_test.rs`**
  Verify frame buffer allocation, dimension checks, and timeout handling.

- [ ] **Step 2: Implement DXGI Desktop Duplication in `src-tauri/src/capture/dxgi.rs`**
  Use `windows::Win32::Graphics::Dxgi::IDXGIOutputDuplication` to acquire next frame, render pointer cursor into the surface, and extract frame data.

- [ ] **Step 3: Implement cursor composite fallback**
  Ensure hardware mouse pointer shape and coordinates are composited into the frame buffer if separate DXGI pointer shape is reported.

- [ ] **Step 4: Run tests and verify frame capture lifecycle**

---

### Task 4: Video Encoding & WebRTC Streaming Engine

**Files:**
- Create: `src-tauri/src/stream/mod.rs`
- Create: `src-tauri/src/stream/encoder.rs`
- Create: `src-tauri/src/stream/webrtc_session.rs`
- Test: `src-tauri/tests/webrtc_stream_test.rs`

**Interfaces:**
- Consumes: `CapturedFrame` from Task 3.
- Produces:
  ```rust
  pub struct StreamConfig {
      pub target_bitrate_kbps: u32,
      pub max_framerate: u32,
  }

  pub struct WebRtcHostSession {
      pub async fn init(config: StreamConfig) -> Result<Self>;
      pub async fn create_offer(&self) -> Result<String>;
      pub async fn apply_answer(&self, sdp: &str) -> Result<()>;
      pub async fn push_frame(&self, frame: CapturedFrame) -> Result<()>;
      pub async fn close(&self) -> Result<()>;
  }
  ```

- [ ] **Step 1: Implement H.264 Encoder abstraction in `src-tauri/src/stream/encoder.rs`**
  Auto-detect GPU hardware encoder (MF / NVENC); fall back to `openh264` software encoder.

- [ ] **Step 2: Implement WebRTC Host Peer Connection in `src-tauri/src/stream/webrtc_session.rs`**
  Using `webrtc` crate: create video track (H.264), configure RTP packetizer, enable PLI / FIR keyframe retransmission handlers.

- [ ] **Step 3: Write integration test `src-tauri/tests/webrtc_stream_test.rs`**
  Verify SDP offer generation, ICE candidate generation, and frame ingestion pipeline.

---

### Task 5: LAN Auto-Discovery (mDNS) & PIN Pairing Security

**Files:**
- Create: `src-tauri/src/discovery/mod.rs`
- Create: `src-tauri/src/discovery/mdns.rs`
- Create: `src-tauri/src/pairing/mod.rs`
- Create: `src-tauri/src/pairing/pin_auth.rs`
- Test: `src-tauri/tests/discovery_pairing_test.rs`

**Interfaces:**
- Produces:
  ```rust
  pub struct PeerDevice {
      pub id: String,
      pub name: String,
      pub ip: std::net::IpAddr,
      pub port: u16,
      pub mode: String, // "host" | "monitor"
  }

  pub trait DiscoveryService {
      fn start_advertising(&self, device_name: &str, port: u16, mode: &str) -> Result<()>;
      fn start_browsing<F>(&self, on_peer_found: F) -> Result<()>
      where F: Fn(PeerDevice) + Send + 'static;
      fn stop(&self) -> Result<()>;
  }
  ```

- [ ] **Step 1: Implement mDNS service in `src-tauri/src/discovery/mdns.rs`**
  Use `mdns-sd` to advertise `_wireless-monitor._tcp.local` and listen for peer nodes.

- [ ] **Step 2: Implement PIN generation and verification in `src-tauri/src/pairing/pin_auth.rs`**
  Generate secure 4-digit numeric PIN; handle timed verification challenge with 3-attempt lockout.

- [ ] **Step 3: Write tests in `src-tauri/tests/discovery_pairing_test.rs`**
  Test correct PIN verification, wrong PIN rejection, and mDNS payload serialization.

---

### Task 6: Native Monitor (Receiver) Rendering Window

**Files:**
- Create: `src-tauri/src/receiver/mod.rs`
- Create: `src-tauri/src/receiver/webrtc_client.rs`
- Create: `src-tauri/src/receiver/surface.rs`
- Test: `src-tauri/tests/receiver_test.rs`

**Interfaces:**
- Consumes: WebRTC stream from Task 4.
- Produces:
  ```rust
  pub struct ReceiverConfig {
      pub fullscreen: bool,
      pub target_display_id: Option<String>,
  }

  pub struct MonitorReceiver {
      pub async fn connect(host_ip: &str, port: u16, pin: &str) -> Result<Self>;
      pub async fn start_presentation(&mut self, config: ReceiverConfig) -> Result<()>;
      pub async fn disconnect(&mut self) -> Result<()>;
  }
  ```

- [ ] **Step 1: Implement WebRTC Client receiver in `src-tauri/src/receiver/webrtc_client.rs`**
  Connect to host signaling server, negotiate SDP answer, receive H.264 RTP stream.

- [ ] **Step 2: Implement native hardware-accelerated presentation window in `src-tauri/src/receiver/surface.rs`**
  Create borderless fullscreen window using `wgpu` or native video surface to render decoded frames with minimal buffering (<1 frame latency).

- [ ] **Step 3: Implement client physical resolution reporting**
  Query local primary monitor width, height, and refresh rate, passing them in the connection handshake.

---

### Task 7: UI Dashboard, Tauri Commands & End-to-End Orchestration

**Files:**
- Modify: `src-tauri/src/main.rs`
- Modify: `src-tauri/src/lib.rs`
- Create: `ui/src/components/DeviceList.ts`
- Create: `ui/src/components/PairingModal.ts`
- Create: `ui/src/components/StatusHeader.ts`
- Modify: `ui/src/main.ts`

**Interfaces:**
- Tauri Commands:
  - `get_device_info() -> DeviceInfo`
  - `start_discovery()`
  - `initiate_extend_display(peer_id: String) -> SessionStatus`
  - `submit_pairing_pin(peer_id: String, pin: String) -> bool`
  - `stop_session()`

- [ ] **Step 1: Implement Tauri IPC commands in `src-tauri/src/lib.rs`**
  Wire frontend commands to `VirtualDisplayManager`, `ScreenCapturer`, `WebRtcHostSession`, and `MonitorReceiver`.

- [ ] **Step 2: Build modern dark-theme UI components in `ui/src/`**
  Display discovered devices, connection status, latency indicators, and PIN prompt dialog.

- [ ] **Step 3: Implement end-to-end teardown safety**
  On application exit, crash, or session disconnect, ensure all created virtual monitors are safely uninstalled/unplugged from Windows.

- [ ] **Step 4: End-to-End integration verification**
  Run full integration verification between host simulation and receiver client.
