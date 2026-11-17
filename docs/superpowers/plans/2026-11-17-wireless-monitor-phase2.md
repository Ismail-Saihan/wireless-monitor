# Wireless Virtual Monitor (Phase 2 Bidirectional & Driver Staging) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement full bidirectional wireless display capability (Linux Host on Wayland → Windows Receiver) and turnkey Windows IddCx driver staging.

**Architecture:** On Linux Host, Wayland virtual displays are dynamically created via D-Bus (GNOME Mutter RemoteDesktop or FreeDesktop XDG Desktop Portal for KDE Plasma 6), screens are captured zero-copy via PipeWire, and streamed over WebRTC. On Windows Receiver, incoming video frames are rendered to a borderless fullscreen Direct3D11 presentation surface with `<1 frame` buffer latency. Pre-compiled WHQL-signed Windows driver release binaries are staged in `drivers/windows/` with verified one-click install/uninstall automation.

**Tech Stack:** Rust 2021, Tauri 2.0, `zbus` (D-Bus), PipeWire screen capture abstractions, Direct3D11 / DXGI (`windows` crate), TypeScript + Tailwind CSS for UI.

**Spec:** [`docs/superpowers/specs/2026-11-17-phase2-bidirectional-design.md`](file:///f:/Wireless%20Monitor%20Project/docs/superpowers/specs/2026-11-17-phase2-bidirectional-design.md)

---

## Global Constraints

- Target latency: Sub-50ms glass-to-glass latency over local 5GHz Wi-Fi / Ethernet.
- Host virtual monitor detection: Must appear in OS Display Settings as an active, configurable monitor.
- Mouse traversal: Host mouse cursor must be captured and rendered directly in the video stream.
- Zero-config discovery: LAN discovery operates via mDNS (`_wireless-monitor._tcp.local`).
- Security: Requires 4-digit PIN authentication before video streaming begins.
- Linux Host scope: Wayland only (GNOME Mutter D-Bus and XDG Desktop Portal for KDE Plasma 6). X11 host mode is out of scope.
- Windows Receiver presentation: Native Direct3D11 dynamic texture and borderless fullscreen presentation queue.

## Review Focus

1. **Non-Wayland / unsupported compositor on Linux Host:** App must detect absence of `$WAYLAND_DISPLAY` or incompatible compositor and return actionable error rather than hanging or panicking.
2. **Display resolution or aspect ratio mismatch on Windows Receiver:** D3D11 presentation surface must dynamically reallocate textures and adjust swap chain dimensions without visual distortion or memory leaks.
3. **PipeWire screencast stream drop / reconnect:** PipeWire capture pipeline must gracefully detect stream closure and notify WebRTC session to trigger keyframe refresh or clean teardown.
4. **Sudden disconnect or crash of Windows Receiver:** Linux Host must detect lost heartbeat and automatically terminate the D-Bus virtual monitor session within 3 seconds to avoid trapping cursor.
5. **Driver installation attempted without Administrator privileges:** Windows driver installer must properly request UAC elevation or report clear elevation error.

---

### Task 1: Linux Wayland Virtual Display Manager (GNOME Mutter & XDG Portal D-Bus)

**Files:**
- Create: `src-tauri/src/display/linux_wayland.rs`
- Modify: `src-tauri/src/display/mod.rs`
- Test: `src-tauri/tests/linux_display_test.rs`
- Verify: `src-tauri/tests/verify_linux_display.py`

**Interfaces:**
- Consumes:
  - `DisplayConfig`, `VirtualDisplayManager` trait from `src-tauri/src/display/mod.rs`
- Produces:
  ```rust
  pub enum WaylandBackend {
      GnomeMutter,
      XdgPortal,
      Simulated,
  }

  pub struct WaylandDisplayManager {
      backend: WaylandBackend,
      active_monitors: std::collections::HashMap<u32, DisplayConfig>,
      next_id: u32,
      // D-Bus session handles
  }

  impl WaylandDisplayManager {
      pub fn detect_backend() -> WaylandBackend;
      pub fn new(backend: WaylandBackend) -> Self;
  }

  impl VirtualDisplayManager for WaylandDisplayManager { ... }
  ```

- [ ] **Step 1: Write the failing unit tests in `src-tauri/tests/linux_display_test.rs`**
  Test backend auto-detection, GNOME Mutter session creation, XDG Portal session creation, invalid resolution rejection, monitor destruction, and cleanup on drop.

- [ ] **Step 2: Implement verification script `src-tauri/tests/verify_linux_display.py`**
  Verify types, traits, backend selection logic, and mock D-Bus message generation.

- [ ] **Step 3: Implement `WaylandDisplayManager` in `src-tauri/src/display/linux_wayland.rs`**
  - Implement `$XDG_CURRENT_DESKTOP` and `$WAYLAND_DISPLAY` environment detection.
  - Implement GNOME Mutter D-Bus calls (`org.gnome.Mutter.RemoteDesktop.CreateSession`, `CreateVirtualMonitor`).
  - Implement FreeDesktop XDG Portal D-Bus calls (`org.freedesktop.portal.RemoteDesktop` & `ScreenCast`).
  - Implement clean teardown and orphan display prevention.

- [ ] **Step 4: Update `src-tauri/src/display/mod.rs`**
  Expose `linux_wayland` module and conditionally alias `DefaultVirtualDisplayManager` for target platforms.

- [ ] **Step 5: Run tests and verification**
  Run: `python src-tauri/tests/verify_linux_display.py`
  Expected: PASS

- [ ] **Step 6: Commit**
  ```bash
  git add src-tauri/src/display/linux_wayland.rs src-tauri/src/display/mod.rs src-tauri/tests/linux_display_test.rs src-tauri/tests/verify_linux_display.py
  git commit -m "feat(display): implement Linux Wayland virtual display manager with Mutter and XDG portal D-Bus"
  ```

---

### Task 2: Linux PipeWire Screen Capture Engine

**Files:**
- Create: `src-tauri/src/capture/pipewire.rs`
- Modify: `src-tauri/src/capture/mod.rs`
- Test: `src-tauri/tests/pipewire_capture_test.rs`
- Verify: `src-tauri/tests/verify_pipewire_capture.py`

**Interfaces:**
- Consumes:
  - `ScreenCapturer`, `CapturedFrame` from `src-tauri/src/capture/mod.rs`
- Produces:
  ```rust
  pub struct PipeWireCapturer {
      node_id: Option<u32>,
      width: u32,
      height: u32,
      is_running: bool,
      last_cursor_pos: Option<(i32, i32)>,
      is_simulated: bool,
  }

  impl PipeWireCapturer {
      pub fn new_simulated(width: u32, height: u32) -> Self;
      pub fn from_node(node_id: u32, width: u32, height: u32) -> Self;
  }

  impl ScreenCapturer for PipeWireCapturer { ... }
  ```

- [ ] **Step 1: Write the failing unit tests in `src-tauri/tests/pipewire_capture_test.rs`**
  Test screencast node stream connection, frame buffer conversion to RGBA, stride alignment, cursor position compositing, and stream termination.

- [ ] **Step 2: Implement verification script `src-tauri/tests/verify_pipewire_capture.py`**
  Verify `PipeWireCapturer` struct, methods, buffer size calculations, and cursor blending logic.

- [ ] **Step 3: Implement `PipeWireCapturer` in `src-tauri/src/capture/pipewire.rs`**
  - Implement screencast node buffer ingestion into `CapturedFrame`.
  - Implement cursor position metadata extraction and alpha compositing.
  - Implement simulated capturer for headless and cross-platform test environments.

- [ ] **Step 4: Update `src-tauri/src/capture/mod.rs`**
  Export `pipewire::PipeWireCapturer`.

- [ ] **Step 5: Run tests and verification**
  Run: `python src-tauri/tests/verify_pipewire_capture.py`
  Expected: PASS

- [ ] **Step 6: Commit**
  ```bash
  git add src-tauri/src/capture/pipewire.rs src-tauri/src/capture/mod.rs src-tauri/tests/pipewire_capture_test.rs src-tauri/tests/verify_pipewire_capture.py
  git commit -m "feat(capture): implement Linux PipeWire screencast capturer with cursor compositing"
  ```

---

### Task 3: Windows Receiver Direct3D11 Presentation Surface

**Files:**
- Create: `src-tauri/src/receiver/windows_surface.rs`
- Modify: `src-tauri/src/receiver/mod.rs`
- Test: `src-tauri/tests/windows_receiver_test.rs`
- Verify: `src-tauri/tests/verify_windows_receiver.py`

**Interfaces:**
- Consumes:
  - `PresentationSurface`, `SurfaceMode`, `SurfaceStats`, `ClientScreenGeometry`, `ReceiverConfig` from `src-tauri/src/receiver/mod.rs`
  - `CapturedFrame` from `src-tauri/src/capture/mod.rs`
- Produces:
  ```rust
  pub struct WindowsD3D11Surface {
      geometry: ClientScreenGeometry,
      config: ReceiverConfig,
      stats: SurfaceStats,
      is_simulated: bool,
  }

  impl WindowsD3D11Surface {
      pub fn new(geometry: ClientScreenGeometry, config: ReceiverConfig) -> Result<Self>;
      pub fn new_simulated(geometry: ClientScreenGeometry, config: ReceiverConfig) -> Self;
      pub fn render_frame(&mut self, frame: &CapturedFrame) -> Result<()>;
      pub fn resize(&mut self, width: u32, height: u32) -> Result<()>;
      pub fn set_fullscreen(&mut self, fullscreen: bool) -> Result<()>;
      pub fn stats(&self) -> SurfaceStats;
  }
  ```

- [ ] **Step 1: Write the failing unit tests in `src-tauri/tests/windows_receiver_test.rs`**
  Test surface initialization, D3D11 dynamic texture allocation, sub-frame latency queue (<1 frame drop logic), resolution re-dimensioning, and borderless fullscreen toggling.

- [ ] **Step 2: Implement verification script `src-tauri/tests/verify_windows_receiver.py`**
  Verify D3D11 surface interfaces, texture upload logic, swap chain presentation queue, and stats tracking.

- [ ] **Step 3: Implement `WindowsD3D11Surface` in `src-tauri/src/receiver/windows_surface.rs`**
  - Direct3D11 device and swap chain initialization logic with `DXGI_FORMAT_B8G8R8A8_UNORM`.
  - Dynamic 2D texture upload with `UpdateSubresource` and borderless presentation.
  - Integration with `<1 frame` buffer queue and dropping stale frames.

- [ ] **Step 4: Update `src-tauri/src/receiver/mod.rs`**
  Export `windows_surface::WindowsD3D11Surface`.

- [ ] **Step 5: Run tests and verification**
  Run: `python src-tauri/tests/verify_windows_receiver.py`
  Expected: PASS

- [ ] **Step 6: Commit**
  ```bash
  git add src-tauri/src/receiver/windows_surface.rs src-tauri/src/receiver/mod.rs src-tauri/tests/windows_receiver_test.rs src-tauri/tests/verify_windows_receiver.py
  git commit -m "feat(receiver): implement Windows Direct3D11 presentation surface with borderless fullscreen"
  ```

---

### Task 4: Windows Virtual Display Driver Packaging & Staging

**Files:**
- Create: `drivers/windows/VirtualDisplayDriver.cer` (staged release certificate)
- Create: `drivers/windows/VirtualDisplayDriver.inf` (staged driver INF manifest)
- Create: `drivers/windows/vdd.exe` (staged companion IPC service binary)
- Create: `drivers/windows/x64/VirtualDisplayDriver.sys` (staged x64 kernel driver)
- Create: `drivers/windows/x64/VirtualDisplayDriver.dll` (staged x64 UMDF driver)
- Modify: `drivers/windows/install_driver.bat`
- Modify: `drivers/windows/uninstall_driver.bat`
- Test: `src-tauri/tests/driver_staging_test.rs`
- Verify: `src-tauri/tests/verify_driver_staging.py`

**Interfaces:**
- Produces: Complete, turnkey driver distribution folder with verified batch installation and clean removal scripts.

- [ ] **Step 1: Write the failing tests in `src-tauri/tests/driver_staging_test.rs`**
  Verify presence, file non-emptiness, architecture integrity (x64), and batch script commands (`certutil`, `pnputil`, `vdd.exe`).

- [ ] **Step 2: Implement verification script `src-tauri/tests/verify_driver_staging.py`**
  Verify manifest files, certificate subject / root trust validation, and uninstallation completeness.

- [ ] **Step 3: Stage release driver binaries and manifests in `drivers/windows/`**
  Stage `VirtualDisplayDriver.inf`, `VirtualDisplayDriver.cer`, `vdd.exe`, and `x64/VirtualDisplayDriver.{sys,dll}`.

- [ ] **Step 4: Update and verify `install_driver.bat` and `uninstall_driver.bat`**
  Ensure scripts properly handle staged layout, UAC elevation, certificate trust store import, and device registration.

- [ ] **Step 5: Run tests and verification**
  Run: `python src-tauri/tests/verify_driver_staging.py`
  Expected: PASS

- [ ] **Step 6: Commit**
  ```bash
  git add drivers/windows/ src-tauri/tests/driver_staging_test.rs src-tauri/tests/verify_driver_staging.py
  git commit -m "feat(drivers): stage Windows Virtual Display Driver release binaries and verify installation automation"
  ```

---

### Task 5: End-to-End Bidirectional Integration & Orchestration

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `ui/src/main.ts`
- Modify: `ui/src/components/StatusHeader.ts`
- Modify: `AGENT.md`
- Test: `src-tauri/tests/e2e_bidirectional_test.rs`
- Verify: `src-tauri/tests/verify_phase2.py`

**Interfaces:**
- Produces:
  - Tauri IPC commands supporting bidirectional mode switching:
    - `get_operating_system() -> String`
    - `set_app_role(role: "host" | "receiver")`
    - `start_linux_host(config: DisplayConfig)`
    - `start_windows_receiver(geometry: ClientScreenGeometry)`
    - `get_phase2_status() -> Phase2Status`

- [ ] **Step 1: Write the failing end-to-end tests in `src-tauri/tests/e2e_bidirectional_test.rs`**
  Test bidirectional transitions: Linux Host creating virtual display and streaming via PipeWire -> Windows Receiver displaying via D3D11 surface; verified PIN handshake and teardown.

- [ ] **Step 2: Implement `src-tauri/tests/verify_phase2.py`**
  Comprehensive test suite verifying all Phase 2 components, types, and bidirectional contracts.

- [ ] **Step 3: Update `src-tauri/src/lib.rs` and `src-tauri/src/main.rs`**
  - Integrate `WaylandDisplayManager` and `PipeWireCapturer` into `AppState`.
  - Integrate `WindowsD3D11Surface` into receiver session management.
  - Expose bidirectional Tauri commands.

- [ ] **Step 4: Update UI in `ui/src/`**
  Enhance `StatusHeader.ts` and dashboard to display active platform (Linux/Windows), dynamic host/receiver toggle, and compositor status.

- [ ] **Step 5: Update `AGENT.md`**
  Document Phase 2 architecture, components, tests, and operational instructions.

- [ ] **Step 6: Run full test suite and verification**
  Run: `python src-tauri/tests/verify_phase2.py`
  Run: `python src-tauri/tests/verify_e2e.py`
  Expected: PASS on all suites!

- [ ] **Step 7: Commit**
  ```bash
  git add src-tauri/ ui/ AGENT.md
  git commit -m "feat: complete Phase 2 bidirectional wireless virtual monitor implementation"
  ```
