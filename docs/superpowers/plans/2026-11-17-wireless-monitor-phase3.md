# Wireless Virtual Monitor (Phase 3 Security, Settings & Packaging) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver mutual pairing approval dialogs, bidirectional mDNS capability discovery, dynamic stream settings (resolution, framerate, bitrate), and turnkey Windows/Linux installer packaging.

**Architecture:** Extend mDNS discovery with DNS TXT capability metadata and implement reverse receiver connection pulling. Upgrade PIN authentication to a full approval state machine with incoming prompt modals, rate-limiting lockout, and countdown expiration. Introduce a configurable `StreamSettings` subsystem driving display creation and video encoder parameters. Configure Tauri 2.0 multi-platform bundler targets (MSI/NSIS with bundled Windows drivers, and `.deb`/AppImage for Linux).

**Tech Stack:** Rust 2021, Tauri 2.0, `mdns-sd`, WebRTC/H.264 abstractions, TypeScript, Tailwind CSS, Python test harnesses.

**Spec:** [`docs/superpowers/specs/2026-11-17-phase3-security-settings-packaging-design.md`](file:///f:/Wireless%20Monitor%20Project/docs/superpowers/specs/2026-11-17-phase3-security-settings-packaging-design.md)

---

## Global Constraints

- **Sub-50ms Glass-to-Glass Latency:** Low latency over local 5GHz Wi-Fi / Ethernet must be preserved across all resolution and bitrate configurations.
- **Mutual Security Verification:** No display stream may be initiated without explicit approval or 4-digit PIN verification.
- **PIN Lockout & Expiry:** PIN challenge must expire after 300 seconds and lock out after 3 failed attempts.
- **Dynamic Resolution Adaptation:** Virtual display creation must adhere to negotiated client screen geometry and user-selected stream settings.
- **Zero Orphaned Displays:** Display destruction hooks and teardown must trigger cleanly on rejection, disconnect, timeout, or application close.
- **Packaging Integrity:** Windows installer must bundle `drivers/windows/` files and execute `install_driver.bat` for turnkey setup.

## Review Focus

1. **Incorrect PIN brute-force attempt:** Attempting more than 3 invalid PIN submissions must trigger immediate session lockout and reject further submissions until a new challenge is generated.
2. **Expired PIN submission:** Submitting a correct PIN after TTL expiration must return an `Expired` error rather than succeeding.
3. **Incoming connection rejection:** When a receiver rejects an incoming connection request, the host must receive an explicit rejection status and tear down any pre-allocated resources.
4. **Invalid stream settings input:** Submitting unsupported resolution dimensions (e.g. 0x0 or odd width/height) or out-of-range framerate must be rejected by validation before display creation.
5. **Driver bundling in Tauri configuration:** The Windows installer configuration must include all files in `drivers/windows/` (`.inf`, `.cer`, `vdd.exe`, `install_driver.bat`, and `x64/` binaries) in the application bundle resources.

---

### Task 1: mDNS Capability Broadcast & Reverse Initiation ("Use as Monitor")

**Files:**
- Modify: `src-tauri/src/discovery/mod.rs`
- Modify: `src-tauri/src/discovery/mdns.rs`
- Modify: `src-tauri/src/lib.rs`
- Create: `src-tauri/tests/discovery_caps_test.rs`

**Interfaces:**
- Consumes:
  - `PeerDevice`, `MdnsDiscovery` from `src-tauri/src/discovery/mod.rs`
- Produces:
  ```rust
  pub struct PeerDevice {
      pub id: String,
      pub name: String,
      pub ip: String,
      pub port: u16,
      pub mode: String,
      pub last_seen: u64,
      pub capabilities: Vec<String>,
      pub max_resolution: String,
      pub max_fps: u32,
  }

  impl MdnsDiscovery {
      pub fn start_advertising_with_caps(
          &self,
          name: &str,
          port: u16,
          mode: &str,
          caps: &[String],
          max_resolution: &str,
          max_fps: u32,
      ) -> anyhow::Result<()>;
  }

  // Tauri IPC command:
  pub fn initiate_use_as_monitor(
      state: tauri::State<'_, AppState>,
      peer_id: String,
      geometry: ClientScreenGeometry,
  ) -> Result<SessionStatus, String>;
  ```

- [x] **Step 1: Write the failing unit tests in `src-tauri/tests/discovery_caps_test.rs`**
  Test advertising with capabilities (TXT records: `caps`, `res`, `fps`), peer parsing with capability fields, backward compatibility for legacy TXT records, and `initiate_use_as_monitor` command workflow.

- [x] **Step 2: Run test to verify it fails**
  Run: `cargo test --manifest-path src-tauri/Cargo.toml --test discovery_caps_test`
  Expected: FAIL with missing fields or methods.

- [x] **Step 3: Implement capabilities in `src-tauri/src/discovery/`**
  - Add `capabilities`, `max_resolution`, and `max_fps` fields to `PeerDevice`.
  - Update `MdnsDiscovery::start_advertising_with_caps` to publish RFC-compliant TXT records.
  - Implement `initiate_use_as_monitor` in `src-tauri/src/lib.rs`.

- [x] **Step 4: Run test to verify it passes**
  Run: `cargo test --manifest-path src-tauri/Cargo.toml --test discovery_caps_test`
  Expected: PASS

- [x] **Step 5: Commit**
  ```bash
  git add src-tauri/src/discovery/ src-tauri/src/lib.rs src-tauri/tests/discovery_caps_test.rs
  git commit -m "feat(discovery): implement mDNS capability broadcast and reverse monitor initiation"
  ```

---

### Task 2: Connection Approval Workflow & Enhanced PIN Security

**Files:**
- Modify: `src-tauri/src/pairing/pin_auth.rs`
- Modify: `src-tauri/src/pairing/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Create: `src-tauri/tests/pairing_security_test.rs`

**Interfaces:**
- Consumes:
  - `PairingState`, `PairingSession`, `PinAuthManager`
- Produces:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
  pub enum PairingState {
      PendingApproval,
      PendingPin,
      Authenticated,
      LockedOut,
      Expired,
      Rejected,
  }

  pub struct IncomingRequest {
      pub peer_id: String,
      pub peer_name: String,
      pub pin: String,
      pub requested_at: u64,
  }

  impl PinAuthManager {
      pub fn request_connection(&mut self, peer_id: &str, peer_name: &str) -> (String, PairingSession);
      pub fn respond_connection(&mut self, peer_id: &str, approved: bool) -> Result<bool, PinAuthError>;
      pub fn get_pending_request(&self, peer_id: &str) -> Option<IncomingRequest>;
      pub fn get_all_pending_requests(&self) -> Vec<IncomingRequest>;
  }

  // Tauri IPC commands:
  pub fn get_incoming_requests(state: tauri::State<'_, AppState>) -> Vec<IncomingRequest>;
  pub fn respond_to_request(state: tauri::State<'_, AppState>, peer_id: String, approved: bool) -> Result<bool, String>;
  ```

- [x] **Step 1: Write the failing unit tests in `src-tauri/tests/pairing_security_test.rs`**
  Test connection request generation, receiver approval/rejection state transitions, 3-attempt lockout enforcement, 300s expiration check, and remaining attempt counters.

- [x] **Step 2: Run test to verify it fails**
  Run: `cargo test --manifest-path src-tauri/Cargo.toml --test pairing_security_test`
  Expected: FAIL

- [x] **Step 3: Implement enhanced pairing state machine in `src-tauri/src/pairing/pin_auth.rs`**
  - Add `PendingApproval` and `Rejected` to `PairingState`.
  - Add `IncomingRequest` tracking and approval methods to `PinAuthManager`.
  - Expose `get_incoming_requests` and `respond_to_request` Tauri commands in `src-tauri/src/lib.rs`.

- [x] **Step 4: Run test to verify it passes**
  Run: `cargo test --manifest-path src-tauri/Cargo.toml --test pairing_security_test`
  Expected: PASS

- [x] **Step 5: Commit**
  ```bash
  git add src-tauri/src/pairing/ src-tauri/src/lib.rs src-tauri/tests/pairing_security_test.rs
  git commit -m "feat(pairing): implement connection approval workflow, incoming request queue and lockout protections"
  ```

---

### Task 3: Stream Settings & Dynamic Resolution Handshake

**Files:**
- Create: `src-tauri/src/stream/settings.rs`
- Modify: `src-tauri/src/stream/mod.rs`
- Modify: `src-tauri/src/stream/encoder.rs`
- Modify: `src-tauri/src/lib.rs`
- Create: `src-tauri/tests/settings_handshake_test.rs`

**Interfaces:**
- Consumes:
  - `DisplayConfig`, `VideoEncoderConfig`
- Produces:
  ```rust
  #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
  pub struct StreamSettings {
      pub resolution_mode: String, // "auto" | "1080p" | "1440p" | "4k" | "custom"
      pub custom_width: Option<u32>,
      pub custom_height: Option<u32>,
      pub framerate: u32,          // 30 | 60
      pub bitrate_kbps: u32,       // 10000 | 20000 | 40000
  }

  impl StreamSettings {
      pub fn resolve_display_config(&self, client_geom: &ClientScreenGeometry) -> DisplayConfig;
      pub fn validate(&self) -> anyhow::Result<()>;
  }

  // Tauri IPC commands:
  pub fn get_stream_settings(state: tauri::State<'_, AppState>) -> StreamSettings;
  pub fn set_stream_settings(state: tauri::State<'_, AppState>, settings: StreamSettings) -> Result<StreamSettings, String>;
  ```

- [x] **Step 1: Write the failing unit tests in `src-tauri/tests/settings_handshake_test.rs`**
  Test default settings resolution, geometry resolution ("auto" adopting client geometry, explicit 1080p/1440p/4k overrides), validation bounds, and dynamic encoder bitrate adjustment.

- [x] **Step 2: Run test to verify it fails**
  Run: `cargo test --manifest-path src-tauri/Cargo.toml --test settings_handshake_test`
  Expected: FAIL

- [x] **Step 3: Implement `StreamSettings` in `src-tauri/src/stream/settings.rs`**
  - Implement struct, validation, and resolution matching logic.
  - Wire settings into `AppState` and expose `get_stream_settings` and `set_stream_settings` commands in `src-tauri/src/lib.rs`.
  - Pass negotiated resolution into `create_display` in `submit_pin`.

- [x] **Step 4: Run test to verify it passes**
  Run: `cargo test --manifest-path src-tauri/Cargo.toml --test settings_handshake_test`
  Expected: PASS

- [x] **Step 5: Commit**
  ```bash
  git add src-tauri/src/stream/ src-tauri/src/lib.rs src-tauri/tests/settings_handshake_test.rs
  git commit -m "feat(stream): implement stream settings management and dynamic resolution handshake"
  ```

---

### Task 4: UI Settings Drawer, Approval Modal & Reverse Initiation

**Files:**
- Create: `ui/src/components/SettingsDrawer.ts`
- Create: `ui/src/components/ApprovalModal.ts`
- Modify: `ui/src/components/DeviceList.ts`
- Modify: `ui/src/components/PairingModal.ts`
- Modify: `ui/src/components/StatusHeader.ts`
- Modify: `ui/src/main.ts`
- Modify: `ui/index.html`

**Interfaces:**
- Consumes:
  - Tauri commands: `get_stream_settings`, `set_stream_settings`, `get_incoming_requests`, `respond_to_request`, `initiate_use_as_monitor`
- Produces:
  - Interactive Settings Drawer with persistence (`localStorage` + Tauri backend).
  - Approval Modal popping up when incoming requests arrive on Receiver.
  - Capability badges and active "Use as Monitor" pull workflow in `DeviceList`.

- [ ] **Step 1: Create `ui/src/components/SettingsDrawer.ts`**
  Implement slide-out settings panel for Resolution (Auto / 1080p / 1440p / 4K), Framerate (30 / 60 FPS), and Bitrate (10 / 20 / 40 Mbps).

- [ ] **Step 2: Create `ui/src/components/ApprovalModal.ts`**
  Implement modal showing incoming connection request, peer name, 4-digit PIN verification code, and Accept / Reject buttons.

- [ ] **Step 3: Update `ui/src/components/DeviceList.ts` & `StatusHeader.ts`**
  - Add Capability tags (e.g., `1080p`, `60 FPS`, `Host/Recv`).
  - Add Settings button/icon in header to trigger Settings Drawer.
  - Wire "Use as Monitor" button to invoke `initiate_use_as_monitor`.

- [ ] **Step 4: Update `ui/src/main.ts` & `ui/index.html`**
  - Integrate `SettingsDrawer` and `ApprovalModal`.
  - Poll for incoming connection requests in receiver mode and display modal automatically.
  - Verify TypeScript compilation: `npx tsc --noEmit`.

- [ ] **Step 5: Commit**
  ```bash
  git add ui/
  git commit -m "feat(ui): implement settings drawer, incoming connection approval modal and reverse initiation"
  ```

---

### Task 5: Packaging Configuration & Distribution Bundling

**Files:**
- Modify: `src-tauri/tauri.conf.json`
- Create: `src-tauri/tests/packaging_test.rs`

**Interfaces:**
- Produces:
  - Windows bundle targets: NSIS and MSI with `drivers/windows/` bundled in application resources.
  - Linux bundle targets: `.deb` and AppImage with desktop file category and library dependencies.

- [ ] **Step 1: Write packaging configuration test in `src-tauri/tests/packaging_test.rs`**
  Test `tauri.conf.json` validity, bundle targets (`nsis`, `msi`, `deb`, `appimage`), resource mapping for `drivers/windows/*`, and app metadata.

- [ ] **Step 2: Run test to verify it fails**
  Run: `cargo test --manifest-path src-tauri/Cargo.toml --test packaging_test`
  Expected: FAIL on missing bundle configuration details.

- [ ] **Step 3: Update `src-tauri/tauri.conf.json`**
  - Add NSIS and MSI targets with driver resource mappings.
  - Add Debian and AppImage configuration with system dependencies.
  - Add bundle identifiers and author metadata.

- [ ] **Step 4: Run test to verify it passes**
  Run: `cargo test --manifest-path src-tauri/Cargo.toml --test packaging_test`
  Expected: PASS

- [ ] **Step 5: Commit**
  ```bash
  git add src-tauri/tauri.conf.json src-tauri/tests/packaging_test.rs
  git commit -m "feat(packaging): configure Windows NSIS/MSI driver bundling and Linux deb/appimage distribution"
  ```

---

### Task 6: Phase 3 Verification Suite & Integration Testing

**Files:**
- Create: `src-tauri/tests/verify_phase3.py`
- Modify: `AGENT.md`
- Modify: `README.md`

**Interfaces:**
- Produces:
  - Python test verification suite executing all unit tests and validating Phase 3 contracts:
    - `test_mdns_capability_broadcast`
    - `test_reverse_monitor_initiation`
    - `test_pairing_approval_and_rejection`
    - `test_pin_lockout_and_expiration`
    - `test_stream_settings_and_auto_resolution`
    - `test_packaging_bundle_configuration`
  - Documentation updates in `AGENT.md` and `README.md`.

- [ ] **Step 1: Implement `src-tauri/tests/verify_phase3.py`**
  Build comprehensive behavioral test suite testing all Phase 3 interfaces and integration points.

- [ ] **Step 2: Run verification script**
  Run: `python src-tauri/tests/verify_phase3.py`
  Expected: PASS (all tests pass)

- [ ] **Step 3: Run existing Phase 1 & 2 suites to ensure zero regressions**
  Run:
  - `python src-tauri/tests/verify_phase2.py`
  - `python src-tauri/tests/verify_e2e.py`
  - `npx tsc --noEmit`
  Expected: ALL PASS

- [ ] **Step 4: Update `AGENT.md` and `README.md` with Phase 3 instructions and status**

- [ ] **Step 5: Commit**
  ```bash
  git add src-tauri/tests/verify_phase3.py AGENT.md README.md
  git commit -m "docs: finalize Phase 3 implementation, test suites, and documentation"
  ```
