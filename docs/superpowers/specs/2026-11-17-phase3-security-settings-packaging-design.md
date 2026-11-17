# Design Document: Phase 3 Pairing Security Polish, Quality & Packaging

**Date:** 2026-11-17  
**Status:** Approved by User & Builder Review  
**Target Repository:** `f:\Wireless Monitor Project`  

---

## 1. Intent & The "Why"

Phase 1 established Windows Host → Linux Receiver streaming, and Phase 2 delivered bidirectional parity (Linux Wayland Host + PipeWire capture ↔ Windows D3D11 Receiver presentation surface, plus staged Windows drivers). However, several critical product-grade capabilities remain incomplete:
1. **Discovery & Reverse Initiation:** While mDNS advertises devices, device capabilities (supported resolutions, refresh rates, active role) are not broadcast in DNS TXT records, and the "Use as Monitor" (pull streaming) button is non-functional in the UI.
2. **Mutual Pairing Security:** When a host connects to a receiver, the receiver currently does not display an incoming connection approval dialog. Both sides must participate in a secure handshake where the user explicitly approves the connection and verifies a 4-digit PIN with lockout protections.
3. **Auto-Resolution & Stream Settings:** Display dimensions currently default statically to 1920x1080@60Hz. Users cannot choose a custom resolution or adjust streaming bitrate caps (e.g. 10 Mbps for congested Wi-Fi, up to 40 Mbps for 5GHz LAN) or target framerate (30 vs 60 FPS).
4. **Distribution Packaging:** The Tauri build configuration lacks production bundling rules for Windows (MSI and NSIS installers with packaged driver scripts) and Linux (Debian `.deb` and AppImage packages).

Phase 3 delivers these enhancements to elevate the application from an experimental prototype to a secure, configurable, distributable product.

---

## 2. Goals, Non-Goals & Anti-Goals

### 2.1 Goals
- **mDNS Capability Broadcast & Reverse Initiation:**
  - Advertise device capabilities (operating system, roles, supported resolutions `1080p,1440p,4k`, refresh rates `30,60`) via RFC-compliant mDNS TXT records.
  - Implement bidirectional connection initiation: Push ("Extend to Laptop") and Pull ("Use as Monitor for PC").
- **Mutual PIN Pairing & Connection Approval Dialog:**
  - When connection is requested, receiver renders an "Incoming Connection Request" modal with peer name, IP, and an Accept / Reject choice.
  - Generates 4-digit PIN with CSPRNG, 300-second TTL, and a 3-attempt lockout.
  - Visual feedback for remaining attempts and countdown timer.
- **Auto-Resolution Handshake & Settings Drawer:**
  - Auto-negotiate client screen geometry during handshake so the virtual display matches client display natively.
  - Provide a UI Settings Drawer allowing users to select:
    - Display Resolution (Native Client Match, 1920x1080, 2560x1440, 3840x2160).
    - Framerate (30 FPS, 60 FPS).
    - Video Bitrate Cap (10 Mbps Eco, 20 Mbps Balanced, 40 Mbps High Quality).
  - Dynamically pass settings into display creation (`DisplayConfig`) and video encoder.
- **Turnkey Production Packaging:**
  - Configure `src-tauri/tauri.conf.json` bundle options for:
    - Windows: NSIS and MSI installers that bundle `drivers/windows/` files and execute `install_driver.bat`.
    - Linux: `.deb` and AppImage bundles with desktop categories and dependencies.
- **Full Verification Suite:**
  - Provide comprehensive test suite `verify_phase3.py` validating capability discovery, PIN challenge lockout, settings negotiation, and packaging configs.

### 2.2 Non-Goals
- **Reverse Input / KVM:** Remote mouse/keyboard control from receiver back to host is out of scope.
- **Audio Streaming:** Audio transmission remains omitted to prioritize sub-50ms glass-to-glass latency.
- **WAN / Cloud Relaying:** Local area network (LAN) only; no internet STUN/TURN servers.

### 2.3 Anti-Goals
- **No Unauthenticated Connections:** No device may start streaming without an explicit PIN verification or user approval.
- **No Fixed-Resolution Constraints:** The virtual monitor must never hardcode 1080p when the client requests higher or lower resolutions.

---

## 3. Approaches Considered & Trade-Offs

| Approach | Design | Trade-Offs | Decision |
| :--- | :--- | :--- | :--- |
| **A. Integrated WebRTC DataChannel Handshake (Selected)** | Exchange resolution, approval requests, and PIN challenge verification over local HTTP/WebSocket or WebRTC signaling before display creation. | Lightweight, direct peer-to-peer, zero external dependencies. | **Accepted** |
| **B. Cloud Broker / Central Signaling Server** | Host a cloud service (e.g. Supabase or WebSocket relay) for device pairing. | Requires WAN access, accounts, violates zero-config local LAN privacy. | **Rejected** |
| **C. Static INI/JSON Configuration File Only** | Configure resolution and bitrate via manual config file edit instead of UI. | Poor UX for non-technical users. | **Rejected** |

---

## 4. System Architecture & Component Design

```
┌────────────────────────────────────────────────────────────────────────┐
│                        PHASE 3 SYSTEM TOPOLOGY                         │
│                                                                        │
│   [DEVICE A (Host or Receiver)]             [DEVICE B (Host or Recv)]  │
│  ┌───────────────────────────┐             ┌─────────────────────────┐ │
│  │ 1. MdnsDiscovery          │             │ 1. MdnsDiscovery        │ │
│  │    TXT: os, mode, res, fps│             │    TXT: os, mode, res   │ │
│  │             │             │             │            │            │ │
│  │             ▼             │             │            ▼            │ │
│  │ 2. Settings Drawer        │             │ 2. Settings Drawer      │ │
│  │    (Res, FPS, Bitrate)    │             │    (Res, FPS, Bitrate)  │ │
│  │             │             │ Handshake   │            │            │ │
│  │             ▼             │ (Res & PIN) │            ▼            │ │
│  │ 3. Connection Coordinator │<───────────>│ 3. Connection Coord     │ │
│  │    - Request / Approve    │             │    - Incoming Dialog    │ │
│  │    - PIN Verification     │             │    - PIN Verification   │ │
│  │             │             │             │            │            │ │
│  │             ▼             │  H.264/RTP  │            ▼            │ │
│  │ 4. Dynamic Display & Enc  │────────────>│ 4. Presentation Surface │ │
│  │    (Configured Res & Bitr)│             │    (Matched Geometry)   │ │
│  └───────────────────────────┘             └─────────────────────────┘ │
└────────────────────────────────────────────────────────────────────────┘
```

### 4.1 Subsystem 1: Capability Broadcast & Reverse Initiation (`discovery/`, `ui/`)
- `MdnsDiscovery::start_advertising_with_caps`: publishes DNS TXT attributes:
  `os=windows`, `role=host,receiver`, `resolutions=1920x1080,2560x1440`, `max_fps=60`.
- In UI `DeviceList.ts`:
  - Displays peer capability badges (`1080p`, `1440p`, `Host/Recv`).
  - "Use as Monitor" button triggers `request_remote_host(peer_id, client_geometry)`.

### 4.2 Subsystem 2: Connection Approval & PIN Security (`pairing/`, `ui/`)
- `PinAuthManager`:
  - CSPRNG 4-digit PIN challenge.
  - Active session states: `PendingApproval`, `PendingPin`, `Authenticated`, `LockedOut`, `Expired`.
  - Rate limiting: max 3 attempts; 30-second penalty lockout or immediate session termination.
- UI `PairingModal.ts` & `ApprovalModal.ts`:
  - Receiver side: Displays "Incoming request from [Peer Name] to extend display. PIN: [ 4 8 2 1 ]. Allow / Reject".
  - Host side: Displays PIN input boxes with countdown timer and attempt counter.

### 4.3 Subsystem 3: Dynamic Stream Settings (`stream/`, `display/`, `ui/`)
- Data Struct `StreamSettings`:
  ```rust
  pub struct StreamSettings {
      pub resolution_mode: String, // "auto" | "1080p" | "1440p" | "4k"
      pub custom_width: Option<u32>,
      pub custom_height: Option<u32>,
      pub framerate: u32,          // 30 | 60
      pub bitrate_kbps: u32,       // 10000 | 20000 | 40000
  }
  ```
- Passed to `create_display(&DisplayConfig)` and `VideoEncoderConfig` for dynamic adaptation.
- UI `SettingsDrawer.ts`: slide-out or modal settings drawer with persistent `localStorage` saving.

### 4.4 Subsystem 4: Distribution Packaging (`tauri.conf.json`, `drivers/`)
- Configure Windows installer:
  - NSIS: embed `drivers/windows/` folder, run `install_driver.bat` on install hook.
  - MSI: target enterprise deployment.
- Configure Linux installer:
  - Debian `.deb` with package dependencies (`libpipewire-0.3-0`, `libdbus-1-3`, `libxdo3`).
  - AppImage bundle.
