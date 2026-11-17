# Design Document: Phase 2 Bidirectional Wireless Virtual Monitor & Driver Staging

**Date:** 2026-11-17  
**Status:** Approved by User & Builder Review  
**Target Repository:** `f:\Wireless Monitor Project`  

---

## 1. Intent & The "Why"

Phase 1 established a functional Windows Host → Linux Receiver baseline (67/67 tests passing). However, true wireless workflow mobility requires bidirectional parity: a user working on a Linux laptop (running Wayland under GNOME or KDE Plasma) must be able to extend their workspace onto a Windows desktop/laptop screen. Furthermore, Windows hosts require a turnkey driver distribution mechanism so users do not have to locate or compile virtual display drivers manually.

Phase 2 completes this vision by delivering:
1. Native Linux Host Mode under Wayland (GNOME Mutter D-Bus and FreeDesktop XDG Desktop Portal for KDE Plasma 6).
2. Native Screen Capture on Linux via PipeWire screencast nodes connected to the WebRTC video streaming pipeline.
3. Native Windows Receiver Mode with Direct3D11 / borderless presentation surface for sub-50ms glass-to-glass playback.
4. Turnkey driver packaging and automated staging for Windows IddCx virtual displays in `drivers/windows/`.

---

## 2. Goals, Non-Goals & Anti-Goals

### 2.1 Goals
- **Wayland Native Virtual Display (Linux Host):** Create an authentic virtual display recognized by GNOME and KDE Display Settings without root privileges or kernel drivers.
- **Auto-Compositor Detection & Standards:** Inspect runtime environment (`$XDG_CURRENT_DESKTOP`, `$WAYLAND_DISPLAY`) to dispatch:
  - **GNOME:** Direct Mutter D-Bus (`org.gnome.Mutter.RemoteDesktop.CreateVirtualMonitor`).
  - **KDE Plasma & Standard Wayland:** FreeDesktop XDG Desktop Portal (`org.freedesktop.portal.RemoteDesktop` & `ScreenCast`).
- **PipeWire Zero-Copy Screen Capture:** Capture the created virtual monitor stream directly via PipeWire screencast nodes and pipe frames to the H.264 WebRTC encoder. Provide simulated capturer for headless/test environments.
- **Windows Receiver Presentation Surface:** Fullscreen low-latency video playback on Windows utilizing Direct3D11 / DXGI dynamic texture presentation with `<1 frame` buffer latency.
- **Turnkey Windows Driver Bundle:** Package signed `Virtual-Display-Driver` release files into `drivers/windows/` with verified installation (`certutil` + `pnputil` + `vdd.exe`) and teardown scripts.
- **Full Test Coverage:** Unit and behavioral verification suites validating Linux D-Bus manager, PipeWire capture contracts, Windows presentation surface, and end-to-end bidirectional handshake.

### 2.2 Non-Goals
- **X11 Host Mode:** Legacy X11 lacks native virtual display isolation without clunky dummy drivers; Linux Host is Wayland-only.
- **Reverse Input / KVM:** Remote touch/mouse control from receiver back to host remains out of scope for Phase 2.
- **Audio Redirection:** Retaining visual-only low-latency streaming pipeline.

### 2.3 Anti-Goals
- **No Sudo / Root Requirements on Linux:** Must not require kernel module compilation (`v4l2loopback`) or root permissions for virtual display creation on Linux.
- **No Windowed Video Player:** Windows Receiver must provide seamless borderless fullscreen display matching native desktop extension.

---

## 3. Approaches Considered & Trade-Offs

| Approach | Design | Trade-Offs | Decision |
| :--- | :--- | :--- | :--- |
| **Option A (Selected)**: Dynamic Runtime Compositor Dispatch & Portal Standard | Probe `$XDG_CURRENT_DESKTOP` to instantiate `GnomeMutterManager` (direct D-Bus) or `XdgPortalManager` (KDE Plasma & Freedesktop standard), sharing a unified `VirtualDisplayManager` trait and PipeWire capture pipeline. | Requires maintaining both Mutter D-Bus and XDG Portal interface adapters, but provides out-of-the-box support for >95% of Linux desktop users without native C dependencies. | **Accepted** |
| **Option B (Rejected)**: GNOME Mutter Exclusive | Implement only GNOME RemoteDesktop D-Bus; defer KDE support. | Leaves KDE users without host functionality despite KDE Plasma 6 having mature virtual output APIs. | **Rejected** |
| **Option C (Rejected)**: `v4l2loopback` / Virtual Framebuffer | Create a virtual video device node in `/dev/video*`. | Requires root/sudo, kernel headers, and fails to create a genuine OS monitor in Wayland display settings. | **Rejected** |

---

## 4. System Architecture & Component Design

```
┌────────────────────────────────────────────────────────────────────────┐
│                   PHASE 2 BIDIRECTIONAL ARCHITECTURE                   │
│                                                                        │
│   [LINUX HOST (Wayland)]                         [WINDOWS RECEIVER]    │
│  ┌───────────────────────────┐                 ┌─────────────────────┐ │
│  │ 1. LinuxDisplayManager    │                 │ 1. Windows Receiver │ │
│  │    - GnomeMutterManager   │                 │    Presentation     │ │
│  │    - XdgPortalManager     │                 │    Surface (D3D11)  │ │
│  │    (via zbus D-Bus)       │                 │                     │ │
│  │             │             │                 │          ▲          │ │
│  │             ▼             │                 │          │          │ │
│  │ 2. PipeWire Capturer      │  WebRTC Stream  │ 2. H264Depacketizer │ │
│  │    (ScreenCast Node Stream│ ──────────────> │    (Sequence check, │ │
│  │     with cursor metadata) │   (RTP/H.264)   │     PLI trigger)    │ │
│  │             │             │                 │          ▲          │ │
│  │             ▼             │                 │          │          │ │
│  │ 3. H.264 VideoEncoder     │                 │ 3. WebRtcClient     │ │
│  │    & WebRtcHostSession    │                 │    (RTP consumer)   │ │
│  │             │             │                 │          ▲          │ │
│  │             ▼             │    Handshake    │          │          │ │
│  │ 4. mDNS & PIN Pairing     │ <─────────────> │ 4. mDNS & PIN Auth  │ │
│  └───────────────────────────┘                 └─────────────────────┘ │
└────────────────────────────────────────────────────────────────────────┘
```

### 4.1 Subsystem 1: Linux Virtual Display Manager (`src-tauri/src/display/linux_wayland.rs`)
Implements `VirtualDisplayManager` trait:
- **Detection:** Checks `$XDG_CURRENT_DESKTOP` and `$WAYLAND_DISPLAY`.
  - If `GNOME` / `ubuntu:GNOME`: Uses `org.gnome.Mutter.RemoteDesktop` D-Bus service.
    - Calls `CreateSession()`, followed by `CreateVirtualMonitor(width, height, refresh_rate)`.
  - If `KDE` / `KDE:KWin` or other Wayland: Uses FreeDesktop XDG Desktop Portal (`org.freedesktop.portal.RemoteDesktop` & `org.freedesktop.portal.ScreenCast`).
- **Lifecycle & Safety:** On drop or `destroy_display()`, closes D-Bus sessions to ensure the virtual output unplugs immediately, preventing orphaned display spaces.

### 4.2 Subsystem 2: Linux Screen Capture via PipeWire (`src-tauri/src/capture/pipewire.rs`)
Implements `ScreenCapturer` trait:
- Connects to the PipeWire screencast stream associated with the created virtual monitor node.
- Ingests frames into RGBA `CapturedFrame` structures, maintaining frame dimensions, timestamp, and buffer size contracts.
- Composites cursor coordinates reported by Mutter/Portal remote desktop metadata.
- Provides a simulated fallback capturer for headless verification and cross-platform CI.

### 4.3 Subsystem 3: Windows Receiver Presentation Surface (`src-tauri/src/receiver/windows_surface.rs`)
- Implements Direct3D11 / DXGI borderless presentation surface on Windows.
- Consumes uncompressed pixel buffers uploaded to dynamic D3D11 textures (`UpdateSubresource`) and presents via swap chain with `<1 frame` buffer queue.
- Supports borderless fullscreen presentation matching the target client display dimensions.

### 4.4 Subsystem 4: Windows Driver Packaging & Staging (`drivers/windows/`)
- Driver folder layout:
  ```
  drivers/windows/
  ├── VirtualDisplayDriver.cer
  ├── VirtualDisplayDriver.inf
  ├── vdd.exe
  ├── x64/
  │   ├── VirtualDisplayDriver.sys
  │   └── VirtualDisplayDriver.dll
  ├── install_driver.bat
  └── uninstall_driver.bat
  ```
- Staging verification:
  1. Validates presence and integrity of all release artifacts.
  2. Ensures `install_driver.bat` imports certificates, installs INF, and registers companion IPC service.
  3. Ensures `uninstall_driver.bat` cleanly tears down device instances, Driver Store packages, and registry keys.

---

## 5. Verification & Testing Strategy
- Unit tests:
  - `src-tauri/tests/linux_display_test.rs`: Mock and test GNOME Mutter and XDG Portal D-Bus session lifecycle, virtual monitor creation, and resolution negotiation.
  - `src-tauri/tests/pipewire_capture_test.rs`: Test PipeWire buffer stream handling, cursor compositing, and timestamp tracking.
  - `src-tauri/tests/windows_receiver_test.rs`: Test Windows D3D11 surface mode, swap chain re-dimensioning, and depacketizer frame ingestion.
  - `src-tauri/tests/driver_staging_test.rs`: Test driver manifest presence, certificate verification logic, and batch script syntax.
- Verification script:
  - `src-tauri/tests/verify_phase2.py`: Comprehensive test suite verifying all Phase 2 traits, structs, D-Bus interfaces, and cross-platform compilation guards.

---

## 6. What Is Left to the Builder
- Choice of mock D-Bus connection backend during automated tests without an active Wayland compositor.
- Exact D3D11 feature level initialization flags (`D3D_FEATURE_LEVEL_11_0` or higher).
- Handling of multiple monitor adapters if the Windows Receiver has multi-monitor setups.
