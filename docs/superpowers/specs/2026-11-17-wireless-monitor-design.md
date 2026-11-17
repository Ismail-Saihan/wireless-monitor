# Design Document: Bidirectional Wireless Virtual Monitor

**Date:** 2026-11-17  
**Status:** Approved by Builder Review & User  
**Target Repository:** `f:\Wireless Monitor Project`

---

## 1. Intent & The "Why"
Users working with multiple machines (e.g., a Windows workstation and a Linux laptop) frequently need a secondary display on the go or at their desk without carrying physical cables or HDMI dummy plugs. Existing solutions are either proprietary, platform-locked (e.g. Apple Sidecar, Samsung Second Screen), or treat the second screen merely as a windowed mirror/remote desktop rather than a genuine, OS-recognized virtual monitor.

This application provides a seamless, open, high-performance, bidirectional wireless monitor experience over a local area network (LAN), enabling either device to extend its desktop onto the other device's screen as a native virtual monitor recognized by OS Display Settings.

---

## 2. Goals, Non-Goals & Anti-Goals

### 2.1 Goals
- **True Virtual Monitor Detection:** The OS must recognize the extended display as an authentic monitor in system display settings (repositioning, resolution adjustment, scaling, and desktop extension).
- **Bidirectional Operation:** Either device (Windows PC or Linux laptop) can serve as the **Host** (generating and extending the virtual display) or the **Monitor** (receiving and displaying the stream fullscreen).
- **Flexible Connection Initiation:** Users can initiate the connection from either device ("Extend to Laptop" from the workstation, or "Use as Monitor for PC" from the laptop).
- **Sub-50ms Glass-to-Glass Latency:** Low latency over local 5GHz Wi-Fi / Ethernet suitable for fluid 60 FPS desktop productivity and coding.
- **Zero-Config LAN Discovery with Security:** Automatic device discovery using mDNS, paired with an approval prompt / 4-digit PIN to prevent unauthorized connections.
- **Unified Native App:** A single, lightweight cross-platform desktop app (Rust + Tauri).
- **Auto-Resolution Matching:** The virtual monitor automatically adopts the exact physical resolution and refresh rate of the receiving client display.

### 2.2 Non-Goals
- **Reverse Input / KVM (v1):** Controlling the host using the receiving laptop's keyboard/trackpad is explicitly out of scope for v1. The host's own mouse and keyboard move onto the virtual screen naturally.
- **Audio Streaming (v1):** Transmitting audio from host to client is omitted to keep streaming synchronization lean and focused solely on visual fidelity.
- **Internet / WAN Streaming:** The app is intended strictly for local area networks (LAN). Cloud relay servers or STUN/TURN servers are not required.
- **X11 Host Support:** Linux Host mode targets modern Wayland compositors (GNOME Mutter & KDE Plasma). X11 as a host is out of scope.

### 2.3 Anti-Goals
- **No Manual HDMI Dummy Plugs:** Must not require physical dummy dongles or hardware loopbacks.
- **No Windowed Mirroring Only:** Must not be a simple desktop mirror/VNC clone that steals the primary display; it must extend the workspace.

---

## 3. Approaches Considered & Trade-Offs

| Approach | Architecture | Pros | Cons | Decision |
| :--- | :--- | :--- | :--- | :--- |
| **A. Electron + WebRTC + Manual Driver** | Node.js / Electron desktop wrapper, WebRTC, user installs driver manually | Fast web UI prototyping | Heavy memory footprint (>150MB idle), clunky manual driver install friction | **Rejected** |
| **B. Python + Qt + GStreamer** | Python PySide6, GStreamer RTSP pipeline | Easy scripting, rapid bindings | Higher CPU overhead, packaging GStreamer on Windows is cumbersome | **Rejected** |
| **C. Rust + Tauri + WebRTC (Chosen)** | Native Rust backend, Tauri webview for UI/settings, native GPU presentation surface (`wgpu`/GStreamer), bundled IddCx driver | Minimal memory (<30MB), high performance, zero-copy DXGI & PipeWire capture, native async networking | Requires Rust toolchain and platform-specific driver/D-Bus bridges | **Selected** |

---

## 4. System Architecture

```
                  LOCAL NETWORK (mDNS Discovery & WebRTC)
 ┌────────────────────────────────────────────────────────────────────────┐
 │                                                                        │
 │  ┌─────────────────────────┐             ┌──────────────────────────┐  │
 │  │      HOST DEVICE        │             │      MONITOR DEVICE      │  │
 │  │                         │             │                          │  │
 │  │ 1. Virtual Display Mgr  │             │ 1. Fullscreen Renderer   │  │
 │  │    (IddCx / Wayland)    │             │    (Native Surface/wgpu) │  │
 │  │            │            │             │            ▲             │  │
 │  │            ▼            │             │            │             │  │
 │  │ 2. Screen Capture       │   WebRTC    │ 2. Hardware Decoder      │  │
 │  │    (DXGI / PipeWire)    │  Stream     │    (DXVA2 / VA-API)      │  │
 │  │            │            │ ──────────> │            ▲             │  │
 │  │            ▼            │  (H.264)    │            │             │  │
 │  │ 3. Hardware Encoder     │             │ 3. WebRTC PeerReceiver   │  │
 │  │    (NVENC/QSV/VA-API)   │             │    (webrtc-rs)           │  │
 │  │                         │ <────────── │                          │  │
 │  │ 4. mDNS (Advertiser     │  Handshake  │ 4. mDNS (Advertiser      │  │
 │  │    & Browser) + PIN     │ & Res Sync  │    & Browser) + PIN      │  │
 │  └─────────────────────────┘             └──────────────────────────┘  │
 └────────────────────────────────────────────────────────────────────────┘
```

### 4.1 Host Mode Subsystems
1. **Virtual Display Creation:**
   - **Windows Host:** Bundles the pre-compiled, WHQL-attestation-signed open-source release of `Virtual-Display-Driver` (by itsmikep / roshkins). On first launch, prompts UAC once to install the driver service. Interacts via named pipe / control API to dynamically plug in a virtual monitor matching client resolution/framerate and unplug on disconnect.
   - **Linux Host (Wayland GNOME/KDE):** Communicates via D-Bus with GNOME Mutter RemoteDesktop (`org.gnome.Mutter.RemoteDesktop.CreateVirtualMonitor`) or KDE Plasma virtual output service to spawn a headless virtual output. No kernel driver or root permissions required.
2. **Display Capture:**
   - **Windows:** DXGI Desktop Duplication API directly captures the virtual monitor adapter output, embedding the hardware mouse cursor.
   - **Linux:** PipeWire screencast node stream produced directly by Mutter/KWin.
3. **Video Encoding:**
   - Hardware-accelerated H.264 encoding via NVENC / Intel QuickSync / AMD AMF on Windows; VA-API / NVENC on Linux.
   - Graceful fallback to software H.264 (OpenH264 / x264 `ultrafast` preset).
4. **WebRTC Streamer:**
   - Peer connection using `webrtc-rs` or native media pipeline.
   - Adaptive bitrate control to adjust dynamically to Wi-Fi jitter.

### 4.2 Monitor (Receiver) Mode Subsystems
1. **LAN Discovery & Initiation:**
   - Both devices run mDNS service advertiser and browser simultaneously (`_wireless-monitor._tcp.local`).
   - Supports Push ("Extend to Laptop") and Pull ("Connect as Monitor").
2. **Handshake & Resolution Sync:**
   - Connects to peer via local signaling (WebSocket / HTTP).
   - Reports physical screen resolution and refresh rate (e.g. 1920x1080 @ 60Hz).
   - Prompts / verifies 4-digit pairing PIN.
3. **Native Presentation & Playback:**
   - Stream playback bypasses WebKitGTK browser limitations on Linux by utilizing a native hardware-accelerated video presentation surface (`wgpu` / GStreamer app sink), guaranteeing sub-50ms glass-to-glass latency and zero stutter.
   - Tauri webview is retained for dashboard UI, settings, and pairing approval modals.

---

## 5. Development Roadmap & Phasing

### Phase 1: Windows Host → Linux Receiver (MVP) — [COMPLETED]
- **1.1:** Bundle WHQL-signed IddCx driver binaries and automate installation & dynamic monitor plug/unplug on Windows.
- **1.2:** Implement DXGI Desktop Duplication capture in Rust for the virtual adapter.
- **1.3:** Setup WebRTC streaming pipeline with H.264 encoding.
- **1.4:** Build Linux Tauri Receiver app with native hardware-accelerated presentation window in fullscreen.
- **1.5:** End-to-end testing: Extend Windows desktop to Linux laptop.

### Phase 2: Linux Host → Windows Receiver (Full Bidirectionality) — [COMPLETED]
*Detailed Spec: [2026-11-17-phase2-bidirectional-design.md](2026-11-17-phase2-bidirectional-design.md)*  
*Detailed Plan: [../plans/2026-11-17-wireless-monitor-phase2.md](../plans/2026-11-17-wireless-monitor-phase2.md)*
- **2.1:** Implement GNOME Mutter & FreeDesktop XDG Desktop Portal (KDE Plasma 6) D-Bus virtual monitor creator.
- **2.2:** Connect PipeWire screencast stream to the WebRTC video encoder on Linux with cursor compositing.
- **2.3:** Build Windows Tauri Receiver Direct3D11 presentation surface for low-latency borderless fullscreen rendering.
- **2.4:** Stage signed Windows driver binaries (`drivers/windows/`) with automated UAC and PnP registration.
- **2.5:** End-to-end testing: Full bidirectional mode switching and automated test suites.

### Phase 3: Pairing Security Polish, Quality & Packaging — [NEXT]
- **3.1:** mDNS bidirectional service discovery on LAN.
- **3.2:** 4-digit PIN pairing and connection approval dialog.
- **3.3:** Auto-resolution handshake and settings panel (resolution, bitrate cap, framerate).
- **3.4:** Installer packaging: MSI / NSIS for Windows, `.deb` / `.tar.gz` / AppImage for Linux.

---

## 6. What Is Left to the Builder
- Detailed Rust module file tree and Tauri IPC command mappings (`create_display`, `start_stream`, `stop_stream`).
- Selection of specific Rust crates (`mdns-sd`, `webrtc`, `windows-rs`, `zbus`, `pipewire`).
- Frontend design styling (clean dark-themed modern UI using Tailwind CSS).
- Error handling strategies for network drops (automatic reconnect / teardown of virtual monitor on timeout).
