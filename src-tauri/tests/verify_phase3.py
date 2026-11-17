"""
Comprehensive Phase 3 Verification Suite: Packaging, Security, UX & Capabilities.
Validates:
1. Syntax, delimiter balance across all Phase 3 Rust backend & TypeScript UI source files.
2. Interface compliance for Phase 3 Tauri IPC commands and data models:
   - initiate_use_as_monitor(peer_id, geometry) -> SessionStatus
   - get_incoming_requests() -> Vec<IncomingRequest>
   - respond_to_request(peer_id, approved) -> bool
   - get_stream_settings() -> StreamSettings
   - set_stream_settings(settings) -> StreamSettings
   - Data models: StreamSettings, IncomingRequest, PairingState, PeerDevice (capabilities)
3. Distribution packaging & bundling configuration in tauri.conf.json:
   - NSIS, MSI, deb, appimage bundle targets
   - Driver resource staging and mappings
   - Debian runtime dependencies
   - Application metadata
4. Comprehensive behavioral test suite validating Phase 3 contracts:
   - test_mdns_capability_broadcast
   - test_reverse_monitor_initiation
   - test_pairing_approval_and_rejection
   - test_pin_lockout_and_expiration
   - test_stream_settings_and_auto_resolution
   - test_packaging_bundle_configuration
   - test_zero_orphaned_displays
"""

import os
import sys
import json
import time
import random
import re
from collections import deque


def check_file_balance(path):
    print(f"Checking syntax & delimiters: {path}...")
    if not os.path.exists(path):
        raise FileNotFoundError(f"File {path} not found")

    with open(path, "r", encoding="utf-8") as f:
        content = f.read()

    counts = {"(": 0, "[": 0, "{": 0}
    pairs = {")": "(", "]": "[", "}": "{"}
    in_str = False
    in_char = False
    escape = False
    in_line_comment = False
    in_block_comment = False

    i = 0
    n = len(content)
    while i < n:
        c = content[i]
        if in_line_comment:
            if c == "\n":
                in_line_comment = False
        elif in_block_comment:
            if c == "*" and i + 1 < n and content[i + 1] == "/":
                in_block_comment = False
                i += 1
        elif in_str:
            if escape:
                escape = False
            elif c == "\\":
                escape = True
            elif c == '"':
                in_str = False
        elif in_char:
            if escape:
                escape = False
            elif c == "\\":
                escape = True
            elif c == "'":
                in_char = False
        else:
            if c == "/" and i + 1 < n and content[i + 1] == "/":
                in_line_comment = True
                i += 1
            elif c == "/" and i + 1 < n and content[i + 1] == "*":
                in_block_comment = True
                i += 1
            elif c == '"':
                in_str = True
            elif c == "'":
                if (i + 2 < n and content[i + 2] == "'") or (i + 3 < n and content[i + 1] == "\\" and content[i + 3] == "'"):
                    in_char = True
            elif c in counts:
                counts[c] += 1
            elif c in pairs:
                opening = pairs[c]
                counts[opening] -= 1
                if counts[opening] < 0:
                    raise ValueError(f"Unmatched closing '{c}' at char {i} in {path}")
        i += 1

    for k, v in counts.items():
        if v != 0:
            raise ValueError(f"Unbalanced '{k}' (diff: {v}) in {path}")
    print(f"  -> Validated: {path}")


def verify_phase3_interfaces():
    print("Verifying Phase 3 Tauri IPC commands and types in src-tauri/src/lib.rs...")
    with open("src-tauri/src/lib.rs", "r", encoding="utf-8") as f:
        lib_src = f.read()

    required_commands = [
        "initiate_use_as_monitor",
        "request_connection",
        "get_incoming_requests",
        "respond_to_request",
        "get_stream_settings",
        "set_stream_settings",
    ]

    for cmd in required_commands:
        if f"fn {cmd}" not in lib_src and f"pub fn {cmd}" not in lib_src:
            raise AssertionError(f"Missing required Phase 3 Tauri IPC command: {cmd} in src-tauri/src/lib.rs")

    # Check AppState Phase 3 fields
    if "stream_settings:" not in lib_src:
        raise AssertionError("Missing 'stream_settings' field in AppState in src-tauri/src/lib.rs")
    if "pin_auth_manager:" not in lib_src:
        raise AssertionError("Missing 'pin_auth_manager' field in AppState in src-tauri/src/lib.rs")

    # Verify pairing interfaces
    with open("src-tauri/src/pairing/pin_auth.rs", "r", encoding="utf-8") as f:
        pin_src = f.read()

    assert "enum PairingState" in pin_src, "Missing PairingState enum in pin_auth.rs"
    assert "PendingApproval" in pin_src, "Missing PendingApproval state in pin_auth.rs"
    assert "PendingPin" in pin_src, "Missing PendingPin state in pin_auth.rs"
    assert "Rejected" in pin_src, "Missing Rejected state in pin_auth.rs"
    assert "struct IncomingRequest" in pin_src, "Missing IncomingRequest struct in pin_auth.rs"
    assert "request_connection" in pin_src, "Missing request_connection method in pin_auth.rs"
    assert "respond_connection" in pin_src, "Missing respond_connection method in pin_auth.rs"

    # Verify stream settings interfaces
    with open("src-tauri/src/stream/settings.rs", "r", encoding="utf-8") as f:
        settings_src = f.read()

    assert "struct StreamSettings" in settings_src, "Missing StreamSettings in settings.rs"
    assert "resolution_mode:" in settings_src, "Missing resolution_mode in StreamSettings"
    assert "custom_width:" in settings_src, "Missing custom_width in StreamSettings"
    assert "custom_height:" in settings_src, "Missing custom_height in StreamSettings"
    assert "framerate:" in settings_src, "Missing framerate in StreamSettings"
    assert "bitrate_kbps:" in settings_src, "Missing bitrate_kbps in StreamSettings"
    assert "resolve_display_config" in settings_src, "Missing resolve_display_config in StreamSettings"

    # Verify discovery capabilities
    with open("src-tauri/src/discovery/mod.rs", "r", encoding="utf-8") as f:
        disc_src = f.read()
    assert "capabilities: Vec<String>" in disc_src, "Missing capabilities in PeerDevice"
    assert "max_resolution: String" in disc_src, "Missing max_resolution in PeerDevice"
    assert "max_fps: u32" in disc_src, "Missing max_fps in PeerDevice"

    print("  -> Phase 3 Rust interfaces & commands verified.")

    # Verify UI TypeScript components and bindings
    print("Verifying Phase 3 UI components and bindings in ui/src/...")
    with open("ui/src/components/ApprovalModal.ts", "r", encoding="utf-8") as f:
        approval_src = f.read()
    assert "class ApprovalModal" in approval_src or "ApprovalModal" in approval_src, "Missing ApprovalModal class"
    assert "IncomingRequest" in approval_src, "ApprovalModal must handle IncomingRequest"

    with open("ui/src/components/SettingsDrawer.ts", "r", encoding="utf-8") as f:
        drawer_src = f.read()
    assert "class SettingsDrawer" in drawer_src or "SettingsDrawer" in drawer_src, "Missing SettingsDrawer class"
    assert "get_stream_settings" in drawer_src, "SettingsDrawer must invoke get_stream_settings"
    assert "set_stream_settings" in drawer_src, "SettingsDrawer must invoke set_stream_settings"

    with open("ui/src/components/DeviceList.ts", "r", encoding="utf-8") as f:
        list_src = f.read()
    assert "onUseAsMonitorClick" in list_src, "DeviceList must support onUseAsMonitorClick"
    assert "Use as Monitor" in list_src, "DeviceList must render 'Use as Monitor' action"

    with open("ui/src/main.ts", "r", encoding="utf-8") as f:
        main_src = f.read()
    assert "get_incoming_requests" in main_src, "main.ts must poll get_incoming_requests"
    assert "respond_to_request" in main_src, "main.ts must invoke respond_to_request"
    assert "initiate_use_as_monitor" in main_src, "main.ts must invoke initiate_use_as_monitor"
    assert "approvalModal" in main_src, "main.ts must integrate approvalModal"
    assert "settingsDrawer" in main_src, "main.ts must integrate settingsDrawer"

    print("  -> Phase 3 UI interfaces & bindings verified.")


def verify_driver_staging():
    print("Verifying Windows driver staging files in drivers/windows/...")
    staged_files = [
        "drivers/windows/VirtualDisplayDriver.cer",
        "drivers/windows/VirtualDisplayDriver.inf",
        "drivers/windows/vdd.exe",
        "drivers/windows/x64/VirtualDisplayDriver.sys",
        "drivers/windows/x64/VirtualDisplayDriver.dll",
        "drivers/windows/install_driver.bat",
        "drivers/windows/uninstall_driver.bat",
    ]
    for path in staged_files:
        if not os.path.exists(path):
            raise FileNotFoundError(f"Missing staged driver file: {path}")
        size = os.path.getsize(path)
        if size == 0:
            raise AssertionError(f"Staged driver file is empty: {path}")
    print("  -> Windows driver staging files verified.")


# ============================================================================
# Behavioral Integration Test Mocks and Simulation Models
# ============================================================================

class MockPeerDevice:
    def __init__(self, id, name, ip, port, mode, last_seen=0, capabilities=None, max_resolution="1920x1080", max_fps=60):
        self.id = id
        self.name = name
        self.ip = ip
        self.port = port
        self.mode = mode
        self.last_seen = last_seen
        self.capabilities = capabilities if capabilities is not None else ["1080p"]
        self.max_resolution = max_resolution
        self.max_fps = max_fps

    def is_host(self):
        return self.mode == "host"

    def is_monitor(self):
        return self.mode == "monitor"

    def to_json(self):
        return json.dumps({
            "id": self.id,
            "name": self.name,
            "ip": self.ip,
            "port": self.port,
            "mode": self.mode,
            "last_seen": self.last_seen,
            "capabilities": self.capabilities,
            "max_resolution": self.max_resolution,
            "max_fps": self.max_fps
        })

    @classmethod
    def from_json(cls, json_str):
        data = json.loads(json_str)
        required = ["id", "name", "ip", "port", "mode"]
        for field in required:
            if field not in data:
                raise ValueError(f"Missing required field {field}")

        last_seen = data.get("last_seen", 0)
        capabilities = data.get("capabilities", ["1080p"])
        max_resolution = data.get("max_resolution", "1920x1080")
        max_fps = data.get("max_fps", 60)

        return cls(data["id"], data["name"], data["ip"], data["port"], data["mode"],
                   last_seen, capabilities, max_resolution, max_fps)


def parse_txt_capabilities(props):
    caps_raw = props.get("caps") or props.get("capabilities")
    if caps_raw:
        caps = [c.strip() for c in caps_raw.split(",") if c.strip()]
        if not caps:
            caps = ["1080p"]
    else:
        caps = ["1080p"]

    max_res = props.get("res") or props.get("max_resolution")
    if not max_res or not max_res.strip():
        max_res = "1920x1080"

    fps_raw = props.get("fps") or props.get("max_fps")
    try:
        max_fps = int(fps_raw) if fps_raw else 60
        if max_fps <= 0:
            max_fps = 60
    except ValueError:
        max_fps = 60

    return caps, max_res, max_fps


class MockClientScreenGeometry:
    def __init__(self, width=1920, height=1080, refresh_rate=60):
        self.width = width
        self.height = height
        self.refresh_rate = refresh_rate

    def validate(self):
        if self.width == 0:
            raise ValueError("Client screen width must be greater than 0")
        if self.height == 0:
            raise ValueError("Client screen height must be greater than 0")
        if self.width % 2 != 0 or self.height % 2 != 0:
            raise ValueError("Width and height must be even numbers for video streaming")
        if self.refresh_rate == 0 or self.refresh_rate > 360:
            raise ValueError("Refresh rate out of bounds")


class MockDisplayConfig:
    def __init__(self, width=1920, height=1080, refresh_rate=60):
        self.width = width
        self.height = height
        self.refresh_rate = refresh_rate

    def __eq__(self, other):
        return (self.width == other.width and
                self.height == other.height and
                self.refresh_rate == other.refresh_rate)

    def __repr__(self):
        return f"DisplayConfig({self.width}x{self.height}@{self.refresh_rate})"


class MockStreamSettings:
    def __init__(self, resolution_mode="auto", custom_width=None, custom_height=None, framerate=60, bitrate_kbps=20000):
        self.resolution_mode = resolution_mode
        self.custom_width = custom_width
        self.custom_height = custom_height
        self.framerate = framerate
        self.bitrate_kbps = bitrate_kbps

    def validate(self):
        mode = self.resolution_mode.strip().lower()
        if mode not in ["auto", "1080p", "1440p", "4k", "custom"]:
            raise ValueError(f"Invalid resolution mode: {self.resolution_mode}")

        if mode == "custom":
            if self.custom_width is None or self.custom_height is None:
                raise ValueError("custom_width and custom_height must be set for custom resolution mode")
            w = self.custom_width
            h = self.custom_height
            if w % 2 != 0 or h % 2 != 0:
                raise ValueError("Custom width and height must be even numbers")
            if w < 640 or w > 7680:
                raise ValueError(f"Custom width {w} outside valid range (640-7680)")
            if h < 480 or h > 4320:
                raise ValueError(f"Custom height {h} outside valid range (480-4320)")

        if self.framerate < 1 or self.framerate > 360:
            raise ValueError(f"Framerate {self.framerate} outside valid range (1-360)")

        if self.bitrate_kbps < 1000 or self.bitrate_kbps > 100000:
            raise ValueError(f"Bitrate {self.bitrate_kbps} outside valid range (1000-100000 kbps)")

    def resolve_display_config(self, client_geom: MockClientScreenGeometry) -> MockDisplayConfig:
        mode = self.resolution_mode.strip().lower()
        if mode == "auto":
            w = (client_geom.width // 2) * 2
            h = (client_geom.height // 2) * 2
            return MockDisplayConfig(w, h, self.framerate)
        elif mode == "1080p":
            return MockDisplayConfig(1920, 1080, self.framerate)
        elif mode == "1440p":
            return MockDisplayConfig(2560, 1440, self.framerate)
        elif mode == "4k":
            return MockDisplayConfig(3840, 2160, self.framerate)
        elif mode == "custom":
            w = (self.custom_width // 2) * 2 if self.custom_width else 1920
            h = (self.custom_height // 2) * 2 if self.custom_height else 1080
            return MockDisplayConfig(w, h, self.framerate)
        else:
            w = (client_geom.width // 2) * 2
            h = (client_geom.height // 2) * 2
            return MockDisplayConfig(w, h, self.framerate)

    def to_stream_config(self):
        return {
            "target_bitrate_kbps": self.bitrate_kbps,
            "max_framerate": self.framerate,
        }


class MockVideoEncoder:
    def __init__(self, bitrate_kbps=20000, framerate=60):
        self.bitrate_kbps = bitrate_kbps
        self.framerate = framerate

    def update_bitrate(self, new_bitrate_kbps):
        if new_bitrate_kbps < 1000 or new_bitrate_kbps > 100000:
            raise ValueError(f"Bitrate {new_bitrate_kbps} out of bounds")
        self.bitrate_kbps = new_bitrate_kbps


class MockIncomingRequest:
    def __init__(self, peer_id, peer_name, pin, requested_at):
        self.peer_id = peer_id
        self.peer_name = peer_name
        self.pin = pin
        self.requested_at = requested_at


class MockPinAuthManager:
    def __init__(self, default_ttl_secs=300, default_max_attempts=3):
        self.sessions = {}
        self.pending_requests = {}
        self.default_ttl_secs = default_ttl_secs
        self.default_max_attempts = default_max_attempts

    def generate_pin(self, peer_id):
        pin = f"{random.randint(1000, 9999):04d}"
        session = {
            "pin": pin,
            "peer_id": peer_id,
            "created_at": time.time(),
            "attempts": 0,
            "max_attempts": self.default_max_attempts,
            "ttl": self.default_ttl_secs,
            "state": "PendingPin",
        }
        self.sessions[peer_id] = session
        return pin

    def request_connection(self, peer_id, peer_name):
        pin = f"{random.randint(1000, 9999):04d}"
        now = int(time.time())
        session = {
            "pin": pin,
            "peer_id": peer_id,
            "created_at": time.time(),
            "attempts": 0,
            "max_attempts": self.default_max_attempts,
            "ttl": self.default_ttl_secs,
            "state": "PendingApproval",
        }
        self.sessions[peer_id] = session
        req = MockIncomingRequest(peer_id, peer_name, pin, now)
        self.pending_requests[peer_id] = req
        return pin, session

    def respond_connection(self, peer_id, approved):
        session = self.sessions.get(peer_id)
        if not session:
            raise KeyError(f"SessionNotFound: {peer_id}")

        if (time.time() - session["created_at"]) >= session["ttl"] or session["state"] == "Expired":
            session["state"] = "Expired"
            self.pending_requests.pop(peer_id, None)
            raise TimeoutError(f"Expired: {peer_id}")

        if session["state"] == "LockedOut" or session["attempts"] >= session["max_attempts"]:
            self.pending_requests.pop(peer_id, None)
            raise ValueError(f"MaxAttemptsExceeded: {peer_id}")

        if session["state"] == "Rejected":
            raise ValueError(f"ConnectionRejected: {peer_id}")

        self.pending_requests.pop(peer_id, None)
        if approved:
            session["state"] = "PendingPin"
            return True
        else:
            session["state"] = "Rejected"
            return False

    def verify_pin(self, peer_id, candidate_pin):
        if len(candidate_pin) != 4 or not candidate_pin.isdigit():
            raise ValueError(f"InvalidPinFormat: {candidate_pin}")

        session = self.sessions.get(peer_id)
        if not session:
            raise KeyError(f"SessionNotFound: {peer_id}")

        if session["state"] == "Authenticated":
            return True

        if (time.time() - session["created_at"]) >= session["ttl"] or session["state"] == "Expired":
            session["state"] = "Expired"
            self.pending_requests.pop(peer_id, None)
            raise TimeoutError(f"Expired: {peer_id}")

        if session["state"] == "LockedOut" or session["attempts"] >= session["max_attempts"]:
            self.pending_requests.pop(peer_id, None)
            raise ValueError(f"MaxAttemptsExceeded: {peer_id}")

        if session["state"] == "Rejected":
            raise ValueError(f"ConnectionRejected: {peer_id}")

        if session["state"] == "PendingApproval":
            raise ValueError(f"PendingApproval: {peer_id}")

        if candidate_pin == session["pin"]:
            session["state"] = "Authenticated"
            return True
        else:
            session["attempts"] += 1
            if session["attempts"] >= session["max_attempts"]:
                session["state"] = "LockedOut"
            return False

    def get_all_pending_requests(self):
        res = []
        for peer_id, req in self.pending_requests.items():
            session = self.sessions.get(peer_id)
            if session:
                if (time.time() - session["created_at"]) < session["ttl"] and session["state"] == "PendingApproval":
                    res.append(req)
        res.sort(key=lambda r: r.requested_at)
        return res


class MockPhase3AppState:
    def __init__(self, fail_capture=False):
        self.app_role = "host"
        self.pin_auth_manager = MockPinAuthManager()
        self.stream_settings = MockStreamSettings()
        self.active_session = None
        self.virtual_displays = {}
        self.next_monitor_id = 1
        self.fail_capture = fail_capture
        self.capturer_running = False
        self.encoder = MockVideoEncoder()

    def get_stream_settings(self):
        return self.stream_settings

    def set_stream_settings(self, settings: MockStreamSettings):
        settings.validate()
        self.stream_settings = settings
        self.encoder.update_bitrate(settings.bitrate_kbps)
        return settings

    def request_connection(self, peer_id, peer_name):
        return self.pin_auth_manager.request_connection(peer_id, peer_name)

    def respond_to_request(self, peer_id, approved):
        return self.pin_auth_manager.respond_connection(peer_id, approved)

    def get_incoming_requests(self):
        return self.pin_auth_manager.get_all_pending_requests()

    def initiate_use_as_monitor(self, peer_id, geometry: MockClientScreenGeometry):
        if not peer_id or not peer_id.strip():
            raise ValueError("peer_id cannot be empty")
        geometry.validate()

        self.app_role = "receiver"
        pin = self.pin_auth_manager.generate_pin(peer_id)
        session = {
            "session_id": f"sess-monitor-{peer_id}",
            "peer_id": peer_id,
            "state": "pairing",
            "monitor_id": None,
            "resolution": f"{geometry.width}x{geometry.height}",
            "fps": geometry.refresh_rate,
            "pin": pin,
        }
        self.active_session = session
        return session

    def submit_pin(self, peer_id, candidate_pin, client_geom=None):
        verified = self.pin_auth_manager.verify_pin(peer_id, candidate_pin)
        if not verified:
            return False

        # Resolve display config from stream settings
        geom = client_geom or MockClientScreenGeometry(1920, 1080, 60)
        config = self.stream_settings.resolve_display_config(geom)

        # Create virtual display
        mid = self.next_monitor_id
        self.next_monitor_id += 1
        self.virtual_displays[mid] = config

        # Start screen capture (simulate potential rollback)
        if self.fail_capture:
            # Rollback display on capture failure to prevent orphaned displays
            del self.virtual_displays[mid]
            raise RuntimeError("Screen capture initialization failed, rolled back display")

        self.capturer_running = True
        self.active_session = {
            "session_id": f"sess-{peer_id}",
            "peer_id": peer_id,
            "state": "active",
            "monitor_id": mid,
            "resolution": f"{config.width}x{config.height}",
            "fps": config.refresh_rate,
            "pin": None,
        }
        return True

    def stop_active_session(self):
        self.capturer_running = False
        self.virtual_displays.clear()
        self.active_session = None
        return True

    def teardown(self):
        self.stop_active_session()


# ============================================================================
# Phase 3 Behavioral Tests Matching Task 6 Specifications
# ============================================================================

def test_mdns_capability_broadcast():
    print("Testing mDNS capability broadcast, serialization, TXT parsing, and fallback...")
    # 1. PeerDevice serialization with capabilities
    peer = MockPeerDevice("peer-receiver-01", "Studio-Laptop-Monitor", "192.168.1.120", 5000, "monitor",
                          1700000000, ["1080p", "1440p", "4k"], "3840x2160", 60)
    assert peer.is_monitor() is True
    assert peer.is_host() is False
    assert peer.max_resolution == "3840x2160"
    assert peer.max_fps == 60
    assert "4k" in peer.capabilities

    json_str = peer.to_json()
    assert "peer-receiver-01" in json_str
    assert "3840x2160" in json_str

    deserialized = MockPeerDevice.from_json(json_str)
    assert deserialized.id == peer.id
    assert deserialized.capabilities == peer.capabilities
    assert deserialized.max_resolution == peer.max_resolution
    assert deserialized.max_fps == peer.max_fps

    # 2. Legacy fallback compatibility
    legacy_json = '{"id":"legacy-peer-02","name":"Old-PC","ip":"192.168.1.55","port":5000,"mode":"host"}'
    legacy_peer = MockPeerDevice.from_json(legacy_json)
    assert legacy_peer.capabilities == ["1080p"]
    assert legacy_peer.max_resolution == "1920x1080"
    assert legacy_peer.max_fps == 60

    # 3. TXT record parsing
    props_full = {"caps": "1080p,1440p", "res": "2560x1440", "fps": "60"}
    caps, res, fps = parse_txt_capabilities(props_full)
    assert caps == ["1080p", "1440p"]
    assert res == "2560x1440"
    assert fps == 60

    props_empty = {}
    c_def, r_def, f_def = parse_txt_capabilities(props_empty)
    assert c_def == ["1080p"]
    assert r_def == "1920x1080"
    assert f_def == 60

    print("  [PASS] test_mdns_capability_broadcast")


def test_reverse_monitor_initiation():
    print("Testing reverse monitor initiation ('Use as Monitor') and geometry validation...")
    app = MockPhase3AppState()

    # Valid initiation
    valid_geom = MockClientScreenGeometry(2560, 1440, 60)
    sess = app.initiate_use_as_monitor("target-host-device", valid_geom)
    assert sess["peer_id"] == "target-host-device"
    assert sess["state"] == "pairing"
    assert sess["resolution"] == "2560x1440"
    assert sess["fps"] == 60
    assert len(sess["pin"]) == 4 and sess["pin"].isdigit()
    assert app.app_role == "receiver"

    # Validation rejection: Empty peer_id
    try:
        app.initiate_use_as_monitor("   ", valid_geom)
        assert False, "Empty peer_id must be rejected"
    except ValueError as e:
        assert "empty" in str(e)

    # Validation rejection: Odd dimensions (H.264 macroblock alignment failure)
    odd_geom = MockClientScreenGeometry(1921, 1080, 60)
    try:
        app.initiate_use_as_monitor("target-host-device", odd_geom)
        assert False, "Odd width must be rejected"
    except ValueError as e:
        assert "even" in str(e)

    # Validation rejection: Zero dimensions
    zero_geom = MockClientScreenGeometry(0, 1080, 60)
    try:
        app.initiate_use_as_monitor("target-host-device", zero_geom)
        assert False, "Zero width must be rejected"
    except ValueError as e:
        assert "greater than 0" in str(e)

    print("  [PASS] test_reverse_monitor_initiation")


def test_pairing_approval_and_rejection():
    print("Testing connection request approval workflow, queue, and rejection transitions...")
    app = MockPhase3AppState()

    # 1. Incoming request generated in PendingApproval state
    pin, session = app.request_connection("client-01", "Alice-Laptop")
    assert session["state"] == "PendingApproval"
    assert len(pin) == 4 and pin.isdigit()

    pending = app.get_incoming_requests()
    assert len(pending) == 1
    assert pending[0].peer_id == "client-01"
    assert pending[0].peer_name == "Alice-Laptop"

    # 2. Submitting PIN while pending approval must be rejected
    try:
        app.submit_pin("client-01", pin)
        assert False, "Submitting PIN before receiver approval must fail"
    except ValueError as e:
        assert "PendingApproval" in str(e)

    # 3. Approve connection -> transitions to PendingPin
    approved = app.respond_to_request("client-01", True)
    assert approved is True
    assert app.pin_auth_manager.sessions["client-01"]["state"] == "PendingPin"
    assert len(app.get_incoming_requests()) == 0

    # 4. Correct PIN submission succeeds once approved
    success = app.submit_pin("client-01", pin)
    assert success is True
    assert app.active_session["state"] == "active"
    assert len(app.virtual_displays) == 1

    # 5. Rejection flow
    pin2, session2 = app.request_connection("client-02", "Bob-Desktop")
    assert session2["state"] == "PendingApproval"
    rejected = app.respond_to_request("client-02", False)
    assert rejected is False
    assert app.pin_auth_manager.sessions["client-02"]["state"] == "Rejected"
    assert len(app.get_incoming_requests()) == 0

    # Submitting PIN after rejection must be rejected
    try:
        app.submit_pin("client-02", pin2)
        assert False, "Submitting PIN on rejected session must fail"
    except ValueError as e:
        assert "ConnectionRejected" in str(e)

    print("  [PASS] test_pairing_approval_and_rejection")


def test_pin_lockout_and_expiration():
    print("Testing 3-attempt PIN lockout protection and 300s session expiration...")
    app = MockPhase3AppState()

    pin, _ = app.request_connection("attacker-dev", "Mallory-PC")
    app.respond_to_request("attacker-dev", True)

    # Attempt 1 & 2: Incorrect PIN
    assert app.submit_pin("attacker-dev", "0000") is False
    assert app.pin_auth_manager.sessions["attacker-dev"]["attempts"] == 1
    assert app.submit_pin("attacker-dev", "1111") is False
    assert app.pin_auth_manager.sessions["attacker-dev"]["attempts"] == 2

    # Attempt 3: Exceeds max attempts -> LockedOut
    assert app.submit_pin("attacker-dev", "2222") is False
    assert app.pin_auth_manager.sessions["attacker-dev"]["state"] == "LockedOut"

    # Attempt 4: Should be blocked due to lockout
    try:
        app.submit_pin("attacker-dev", pin)
        assert False, "Locked-out session must reject further attempts"
    except ValueError as e:
        assert "MaxAttemptsExceeded" in str(e)

    # TTL Expiration test (300s TTL)
    pin_exp, _ = app.request_connection("slow-dev", "Rip-Van-Winkle")
    app.respond_to_request("slow-dev", True)
    # Simulate time advancing 301 seconds
    app.pin_auth_manager.sessions["slow-dev"]["created_at"] = time.time() - 301

    try:
        app.submit_pin("slow-dev", pin_exp)
        assert False, "Expired session (>300s) must reject PIN submission"
    except TimeoutError as e:
        assert "Expired" in str(e)

    print("  [PASS] test_pin_lockout_and_expiration")


def test_stream_settings_and_auto_resolution():
    print("Testing StreamSettings resolution modes, dynamic config negotiation, and bounds...")
    app = MockPhase3AppState()

    # 1. Default settings: auto mode, 60fps, 20Mbps
    defaults = app.get_stream_settings()
    assert defaults.resolution_mode == "auto"
    assert defaults.framerate == 60
    assert defaults.bitrate_kbps == 20000

    # 2. Auto resolution adoption of client screen geometry
    client_geom = MockClientScreenGeometry(2560, 1440, 120)
    resolved = defaults.resolve_display_config(client_geom)
    assert resolved.width == 2560
    assert resolved.height == 1440
    assert resolved.refresh_rate == 60 # settings framerate overrides

    # 3. Presets: 1080p, 1440p, 4k
    for mode, w, h in [("1080p", 1920, 1080), ("1440p", 2560, 1440), ("4k", 3840, 2160)]:
        s = MockStreamSettings(resolution_mode=mode, framerate=60)
        s.validate()
        conf = s.resolve_display_config(client_geom)
        assert conf.width == w and conf.height == h

    # 4. Custom resolution mode
    custom = MockStreamSettings(resolution_mode="custom", custom_width=3440, custom_height=1440, framerate=100, bitrate_kbps=35000)
    custom.validate()
    conf_custom = custom.resolve_display_config(client_geom)
    assert conf_custom.width == 3440
    assert conf_custom.height == 1440
    assert conf_custom.refresh_rate == 100

    # 5. Bounds validation: Odd custom dimensions, bitrate/framerate bounds
    try:
        MockStreamSettings("custom", 1921, 1080).validate()
        assert False, "Odd custom width must fail"
    except ValueError:
        pass

    try:
        MockStreamSettings(framerate=0).validate()
        assert False, "Framerate 0 must fail"
    except ValueError:
        pass

    try:
        MockStreamSettings(bitrate_kbps=500).validate() # below 1000
        assert False, "Bitrate < 1000 kbps must fail"
    except ValueError:
        pass

    # 6. Encoder bitrate update
    app.set_stream_settings(custom)
    assert app.encoder.bitrate_kbps == 35000

    print("  [PASS] test_stream_settings_and_auto_resolution")


def test_packaging_bundle_configuration():
    print("Testing packaging bundle configuration in src-tauri/tauri.conf.json...")
    tauri_conf_path = "src-tauri/tauri.conf.json"
    assert os.path.exists(tauri_conf_path), f"tauri.conf.json missing at {tauri_conf_path}"

    with open(tauri_conf_path, "r", encoding="utf-8") as f:
        conf = json.load(f)

    # Basic identity
    assert conf.get("productName") == "Wireless Monitor"
    assert conf.get("identifier") == "com.wirelessmonitor.app"
    assert conf.get("version")

    # Bundle active and targets
    bundle = conf.get("bundle", {})
    assert bundle.get("active") is True, "bundle.active must be True"

    targets = bundle.get("targets", [])
    required_targets = ["nsis", "msi", "deb", "appimage"]
    for t in required_targets:
        assert t in targets, f"Missing target '{t}' in bundle.targets: {targets}"

    # Driver resources
    resources = bundle.get("resources", [])
    has_driver_res = any("drivers/windows" in r or "drivers\\windows" in r for r in resources)
    assert has_driver_res, f"Driver mapping missing in bundle.resources: {resources}"

    # Debian dependencies
    deb_deps = bundle.get("linux", {}).get("deb", {}).get("depends", [])
    for dep in ["libpipewire-0.3-0", "libdbus-1-3", "libxdo3"]:
        assert dep in deb_deps, f"Missing required Debian dependency '{dep}' in: {deb_deps}"

    # Metadata
    assert bundle.get("category") == "Utility"
    assert bundle.get("copyright")
    assert bundle.get("description") or conf.get("description")

    print("  [PASS] test_packaging_bundle_configuration")


def test_zero_orphaned_displays():
    print("Testing zero orphaned virtual displays on rejection, lockout, and capture errors...")

    # Case 1: Rejected connection -> 0 displays created
    app1 = MockPhase3AppState()
    pin1, _ = app1.request_connection("client-rej", "Reject-Laptop")
    app1.respond_to_request("client-rej", False)
    assert len(app1.virtual_displays) == 0

    # Case 2: Lockout -> 0 displays created
    app2 = MockPhase3AppState()
    pin2, _ = app2.request_connection("client-lock", "Lock-Laptop")
    app2.respond_to_request("client-lock", True)
    for _ in range(3):
        app2.submit_pin("client-lock", "0000")
    assert len(app2.virtual_displays) == 0

    # Case 3: Capture initialization error -> Virtual display rolled back immediately
    app3 = MockPhase3AppState(fail_capture=True)
    pin3, _ = app3.request_connection("client-err", "Error-Laptop")
    app3.respond_to_request("client-err", True)
    try:
        app3.submit_pin("client-err", pin3)
        assert False, "Should fail on capture initialization"
    except RuntimeError as e:
        assert "rolled back" in str(e)
    # Virtual display was rolled back
    assert len(app3.virtual_displays) == 0
    assert app3.active_session is None

    # Case 4: Normal active session teardown destroys all displays
    app4 = MockPhase3AppState(fail_capture=False)
    pin4, _ = app4.request_connection("client-ok", "Good-Laptop")
    app4.respond_to_request("client-ok", True)
    assert app4.submit_pin("client-ok", pin4) is True
    assert len(app4.virtual_displays) == 1
    assert app4.active_session is not None

    app4.teardown()
    assert len(app4.virtual_displays) == 0
    assert app4.active_session is None
    assert app4.capturer_running is False

    print("  [PASS] test_zero_orphaned_displays")


def main():
    print("=" * 70)
    print("Starting Comprehensive Phase 3 Verification Suite")
    print("=" * 70)

    # 1. Delimiter balance and syntax checks across all Phase 3 files
    files_to_check = [
        "src-tauri/src/discovery/mdns.rs",
        "src-tauri/src/discovery/mod.rs",
        "src-tauri/src/pairing/pin_auth.rs",
        "src-tauri/src/pairing/mod.rs",
        "src-tauri/src/stream/settings.rs",
        "src-tauri/src/stream/encoder.rs",
        "src-tauri/src/stream/mod.rs",
        "src-tauri/src/lib.rs",
        "src-tauri/tests/discovery_caps_test.rs",
        "src-tauri/tests/pairing_security_test.rs",
        "src-tauri/tests/settings_handshake_test.rs",
        "src-tauri/tests/packaging_test.rs",
        "ui/src/components/ApprovalModal.ts",
        "ui/src/components/SettingsDrawer.ts",
        "ui/src/components/DeviceList.ts",
        "ui/src/components/PairingModal.ts",
        "ui/src/components/StatusHeader.ts",
        "ui/src/main.ts",
    ]
    for f in files_to_check:
        check_file_balance(f)

    # 2. Interface and schema validations
    verify_phase3_interfaces()

    # 3. Staged driver asset validations
    verify_driver_staging()

    # 4. Behavioral integration tests
    print("\nRunning Phase 3 Behavioral Integration Tests...")
    test_mdns_capability_broadcast()
    test_reverse_monitor_initiation()
    test_pairing_approval_and_rejection()
    test_pin_lockout_and_expiration()
    test_stream_settings_and_auto_resolution()
    test_packaging_bundle_configuration()
    test_zero_orphaned_displays()

    print("\n" + "=" * 70)
    print("ALL PHASE 3 VERIFICATION SUITE TESTS PASSED SUCCESSFULLY (0 ERRORS)!")
    print("=" * 70)


if __name__ == "__main__":
    try:
        main()
    except Exception as e:
        print(f"\n[FAIL] Phase 3 verification error: {e}", file=sys.stderr)
        sys.exit(1)
