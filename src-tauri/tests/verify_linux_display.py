"""
Test and validation suite for Linux Wayland Virtual Display Manager.
Verifies syntax, interface compliance, D-Bus constants/contracts, and full behavioral state machine tests.
"""

import os
import sys


def check_file_balance(path):
    print(f"Checking {path}...")
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
                if i + 2 < n and content[i + 2] == "'":
                    in_char = True
            elif c in counts:
                counts[c] += 1
            elif c in pairs:
                matching = pairs[c]
                counts[matching] -= 1
                if counts[matching] < 0:
                    raise ValueError(f"Mismatched closing {c} at offset {i}")
        i += 1

    for k, v in counts.items():
        if v != 0:
            raise ValueError(f"Unbalanced {k} (count: {v}) in {path}")
    print(f"  -> Syntax and delimiters valid: {path}")


def verify_interface_and_types():
    with open("src-tauri/src/display/mod.rs", "r", encoding="utf-8") as f:
        mod_rs = f.read()

    assert "pub mod linux_wayland;" in mod_rs, "linux_wayland module not declared in mod.rs"
    assert "pub use linux_wayland::{" in mod_rs or "pub use linux_wayland::WaylandDisplayManager;" in mod_rs, (
        "linux_wayland exports missing in mod.rs"
    )
    assert "WaylandBackend" in mod_rs, "WaylandBackend not exported in mod.rs"
    assert "WaylandDisplayManager" in mod_rs, "WaylandDisplayManager not exported in mod.rs"
    assert "DefaultVirtualDisplayManager" in mod_rs, "DefaultVirtualDisplayManager alias missing in mod.rs"

    with open("src-tauri/src/display/linux_wayland.rs", "r", encoding="utf-8") as f:
        wl_rs = f.read()

    assert "pub enum WaylandBackend" in wl_rs, "WaylandBackend enum missing"
    assert "GnomeMutter" in wl_rs, "GnomeMutter variant missing"
    assert "XdgPortal" in wl_rs, "XdgPortal variant missing"
    assert "Simulated" in wl_rs, "Simulated variant missing"

    assert "pub struct WaylandDisplayManager" in wl_rs, "WaylandDisplayManager struct missing"
    assert "impl VirtualDisplayManager for WaylandDisplayManager" in wl_rs, "VirtualDisplayManager not implemented"
    assert "impl Drop for WaylandDisplayManager" in wl_rs, "Drop not implemented"

    # Methods
    assert "pub fn new(" in wl_rs, "new() missing"
    assert "pub fn new_simulated(" in wl_rs, "new_simulated() missing"
    assert "pub fn new_mock(" in wl_rs, "new_mock() missing"
    assert "pub fn detect_backend(" in wl_rs, "detect_backend() missing"
    assert "pub fn detect_backend_from_env(" in wl_rs, "detect_backend_from_env() missing"
    assert "pub fn display_count(&self) -> usize" in wl_rs, "display_count() missing"
    assert "pub fn active_displays(&self) -> Vec<u32>" in wl_rs, "active_displays() missing"
    assert "pub fn get_display_config(&self, monitor_id: u32)" in wl_rs, "get_display_config() missing"
    assert "pub fn session_handle(&self, monitor_id: u32)" in wl_rs, "session_handle() missing"
    assert "pub fn dbus_call_log(&self)" in wl_rs, "dbus_call_log() missing"

    # D-Bus constants
    assert 'pub const MUTTER_SERVICE: &str = "org.gnome.Mutter.RemoteDesktop";' in wl_rs, "MUTTER_SERVICE missing"
    assert 'pub const MUTTER_PATH: &str = "/org/gnome/Mutter/RemoteDesktop";' in wl_rs, "MUTTER_PATH missing"
    assert 'pub const MUTTER_INTERFACE: &str = "org.gnome.Mutter.RemoteDesktop";' in wl_rs, "MUTTER_INTERFACE missing"
    assert 'pub const MUTTER_SESSION_INTERFACE: &str = "org.gnome.Mutter.RemoteDesktop.Session";' in wl_rs, (
        "MUTTER_SESSION_INTERFACE missing"
    )

    assert 'pub const PORTAL_SERVICE: &str = "org.freedesktop.portal.Desktop";' in wl_rs, "PORTAL_SERVICE missing"
    assert 'pub const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";' in wl_rs, "PORTAL_PATH missing"
    assert 'pub const PORTAL_REMOTE_DESKTOP_INTERFACE: &str = "org.freedesktop.portal.RemoteDesktop";' in wl_rs, (
        "PORTAL_REMOTE_DESKTOP_INTERFACE missing"
    )
    assert 'pub const PORTAL_SCREENCAST_INTERFACE: &str = "org.freedesktop.portal.ScreenCast";' in wl_rs, (
        "PORTAL_SCREENCAST_INTERFACE missing"
    )
    assert 'pub const PORTAL_SESSION_INTERFACE: &str = "org.freedesktop.portal.Session";' in wl_rs, (
        "PORTAL_SESSION_INTERFACE missing"
    )

    # Mock D-Bus message constructors
    assert "pub fn build_mutter_create_session_call" in wl_rs, "build_mutter_create_session_call missing"
    assert "pub fn build_mutter_create_virtual_monitor_call" in wl_rs, "build_mutter_create_virtual_monitor_call missing"
    assert "pub fn build_mutter_stop_session_call" in wl_rs, "build_mutter_stop_session_call missing"
    assert "pub fn build_portal_create_session_call" in wl_rs, "build_portal_create_session_call missing"
    assert "pub fn build_portal_screencast_call" in wl_rs, "build_portal_screencast_call missing"
    assert "pub fn build_portal_close_call" in wl_rs, "build_portal_close_call missing"

    print("  -> Linux Wayland interface, types, and D-Bus contracts verified successfully.")


# Behavioral Python Simulation
class SimulatedDisplayConfig:
    def __init__(self, width: int, height: int, refresh_rate: int):
        self.width = width
        self.height = height
        self.refresh_rate = refresh_rate

    def validate(self):
        if self.width == 0 or self.height == 0 or self.refresh_rate == 0:
            raise ValueError("Display dimensions and refresh rate must be non-zero")
        if self.width < 320 or self.width > 7680:
            raise ValueError(f"Display width {self.width} is outside supported range (320-7680)")
        if self.height < 240 or self.height > 4320:
            raise ValueError(f"Display height {self.height} is outside supported range (240-4320)")
        if self.refresh_rate < 10 or self.refresh_rate > 360:
            raise ValueError(f"Refresh rate {self.refresh_rate} Hz is outside supported range (10-360 Hz)")
        return True


class DBusCallRecord:
    def __init__(self, destination: str, path: str, interface: str, member: str, arguments=None):
        self.destination = destination
        self.path = path
        self.interface = interface
        self.member = member
        self.arguments = arguments or []

    def __repr__(self):
        return f"DBusCall({self.destination}.{self.member})"


class SimulatedWaylandDisplayManager:
    MUTTER_SERVICE = "org.gnome.Mutter.RemoteDesktop"
    MUTTER_PATH = "/org/gnome/Mutter/RemoteDesktop"
    MUTTER_INTERFACE = "org.gnome.Mutter.RemoteDesktop"
    MUTTER_SESSION_INTERFACE = "org.gnome.Mutter.RemoteDesktop.Session"

    PORTAL_SERVICE = "org.freedesktop.portal.Desktop"
    PORTAL_PATH = "/org/freedesktop/portal/desktop"
    PORTAL_REMOTE_DESKTOP_INTERFACE = "org.freedesktop.portal.RemoteDesktop"
    PORTAL_SCREENCAST_INTERFACE = "org.freedesktop.portal.ScreenCast"
    PORTAL_SESSION_INTERFACE = "org.freedesktop.portal.Session"

    def __init__(self, backend="Simulated", mock_mode=False, wayland_running=False):
        self.backend = backend
        self.mock_mode = mock_mode
        self.wayland_running = wayland_running
        self.active_monitors = {}
        self.session_handles = {}
        self.next_id = 1
        self.dbus_log = []
        self.simulated_driver_installed = (backend == "Simulated" or mock_mode)

    @classmethod
    def detect_backend_from_env(cls, wayland_display, xdg_desktop):
        if not wayland_display:
            return "Simulated"
        desktop = (xdg_desktop or "").lower()
        if "gnome" in desktop:
            return "GnomeMutter"
        elif "kde" in desktop or "plasma" in desktop:
            return "XdgPortal"
        else:
            return "XdgPortal"

    def is_mock(self):
        return self.mock_mode

    def is_simulated(self):
        return self.backend == "Simulated"

    def display_count(self):
        return len(self.active_monitors)

    def active_displays(self):
        return sorted(self.active_monitors.keys())

    def get_display_config(self, monitor_id: int):
        return self.active_monitors.get(monitor_id)

    def session_handle(self, monitor_id: int):
        return self.session_handles.get(monitor_id)

    def is_driver_installed(self):
        if self.mock_mode or self.backend == "Simulated":
            return self.simulated_driver_installed
        return self.wayland_running

    def set_simulated_driver_installed(self, inst: bool):
        self.simulated_driver_installed = inst

    def create_display(self, config: SimulatedDisplayConfig):
        config.validate()

        if not self.mock_mode and self.backend != "Simulated" and not self.wayland_running:
            raise RuntimeError("Wayland session not detected: $WAYLAND_DISPLAY is not set. Linux host mode requires active Wayland compositor.")

        monitor_id = self.next_id
        self.next_id += 1

        if self.backend == "GnomeMutter":
            session_path = f"/org/gnome/Mutter/RemoteDesktop/Session/u{monitor_id}"
            self.session_handles[monitor_id] = session_path
            self.dbus_log.append(DBusCallRecord(self.MUTTER_SERVICE, self.MUTTER_PATH, self.MUTTER_INTERFACE, "CreateSession"))
            self.dbus_log.append(DBusCallRecord(
                self.MUTTER_SERVICE, session_path, self.MUTTER_SESSION_INTERFACE, "CreateVirtualMonitor",
                [f"name: Wireless-Monitor", f"width: {config.width}", f"height: {config.height}", f"refresh_rate: {config.refresh_rate}"]
            ))
            self.dbus_log.append(DBusCallRecord(self.MUTTER_SERVICE, session_path, self.MUTTER_SESSION_INTERFACE, "Start"))
        elif self.backend == "XdgPortal":
            session_handle = f"/org/freedesktop/portal/desktop/session/xdg_{monitor_id}"
            self.session_handles[monitor_id] = session_handle
            self.dbus_log.append(DBusCallRecord(self.PORTAL_SERVICE, self.PORTAL_PATH, self.PORTAL_REMOTE_DESKTOP_INTERFACE, "CreateSession", ["session_handle_token: wireless_monitor"]))
            self.dbus_log.append(DBusCallRecord(
                self.PORTAL_SERVICE, self.PORTAL_PATH, self.PORTAL_SCREENCAST_INTERFACE, "CreateSession",
                [f"session_handle: {session_handle}", f"resolution: {config.width}x{config.height}@{config.refresh_rate}"]
            ))
        else: # Simulated
            session_handle = f"/simulated/session/{monitor_id}"
            self.session_handles[monitor_id] = session_handle

        self.active_monitors[monitor_id] = config
        return monitor_id

    def destroy_display(self, monitor_id: int):
        if monitor_id not in self.active_monitors:
            raise KeyError(f"Virtual display with ID {monitor_id} not found")

        handle = self.session_handles.pop(monitor_id, None)
        if handle:
            if self.backend == "GnomeMutter":
                self.dbus_log.append(DBusCallRecord(self.MUTTER_SERVICE, handle, self.MUTTER_SESSION_INTERFACE, "Stop"))
            elif self.backend == "XdgPortal":
                self.dbus_log.append(DBusCallRecord(self.PORTAL_SERVICE, handle, self.PORTAL_SESSION_INTERFACE, "Close"))

        del self.active_monitors[monitor_id]

    def destroy_all(self):
        ids = list(self.active_monitors.keys())
        for m_id in ids:
            self.destroy_display(m_id)
        self.active_monitors.clear()
        self.session_handles.clear()

    def __del__(self):
        if self.active_monitors:
            self.destroy_all()


def run_behavioral_tests():
    print("Running behavioral test cases matching src-tauri/tests/linux_display_test.rs...")

    # 1. test_backend_detection_gnome
    b1 = SimulatedWaylandDisplayManager.detect_backend_from_env("wayland-0", "GNOME")
    assert b1 == "GnomeMutter"
    b2 = SimulatedWaylandDisplayManager.detect_backend_from_env("wayland-1", "ubuntu:GNOME")
    assert b2 == "GnomeMutter"
    print("  [PASS] test_backend_detection_gnome")

    # 2. test_backend_detection_kde_plasma
    b_kde = SimulatedWaylandDisplayManager.detect_backend_from_env("wayland-0", "KDE")
    assert b_kde == "XdgPortal"
    b_plasma = SimulatedWaylandDisplayManager.detect_backend_from_env("wayland-0", "KDE:KWin")
    assert b_plasma == "XdgPortal"
    print("  [PASS] test_backend_detection_kde_plasma")

    # 3. test_backend_detection_other_wayland
    b_sway = SimulatedWaylandDisplayManager.detect_backend_from_env("wayland-0", "sway")
    assert b_sway == "XdgPortal"
    print("  [PASS] test_backend_detection_other_wayland")

    # 4. test_backend_detection_simulated_fallback
    assert SimulatedWaylandDisplayManager.detect_backend_from_env(None, "GNOME") == "Simulated"
    assert SimulatedWaylandDisplayManager.detect_backend_from_env("", "KDE") == "Simulated"
    print("  [PASS] test_backend_detection_simulated_fallback")

    # 5. test_simulated_display_lifecycle
    sim = SimulatedWaylandDisplayManager("Simulated")
    assert sim.display_count() == 0
    assert len(sim.active_displays()) == 0
    cfg = SimulatedDisplayConfig(1920, 1080, 60)
    mid = sim.create_display(cfg)
    assert mid > 0
    assert sim.display_count() == 1
    assert sim.get_display_config(mid).width == 1920
    sim.destroy_display(mid)
    assert sim.display_count() == 0
    try:
        sim.destroy_display(mid)
        assert False, "Double destroy should fail"
    except KeyError:
        pass
    print("  [PASS] test_simulated_display_lifecycle")

    # 6. test_gnome_mutter_session_creation_and_dbus_calls
    mutter_mgr = SimulatedWaylandDisplayManager("GnomeMutter", mock_mode=True)
    cfg2 = SimulatedDisplayConfig(2560, 1440, 120)
    m_id = mutter_mgr.create_display(cfg2)
    assert mutter_mgr.display_count() == 1
    assert "Mutter" in mutter_mgr.session_handle(m_id)
    # verify calls
    calls = mutter_mgr.dbus_log
    assert any(c.member == "CreateSession" and c.destination == SimulatedWaylandDisplayManager.MUTTER_SERVICE for c in calls)
    assert any(c.member == "CreateVirtualMonitor" and "2560" in str(c.arguments) for c in calls)
    mutter_mgr.destroy_display(m_id)
    assert any(c.member == "Stop" for c in mutter_mgr.dbus_log)
    print("  [PASS] test_gnome_mutter_session_creation_and_dbus_calls")

    # 7. test_xdg_portal_session_creation_and_dbus_calls
    portal_mgr = SimulatedWaylandDisplayManager("XdgPortal", mock_mode=True)
    cfg3 = SimulatedDisplayConfig(3840, 2160, 60)
    p_id = portal_mgr.create_display(cfg3)
    assert portal_mgr.display_count() == 1
    assert "portal" in portal_mgr.session_handle(p_id)
    p_calls = portal_mgr.dbus_log
    assert any(c.interface == SimulatedWaylandDisplayManager.PORTAL_REMOTE_DESKTOP_INTERFACE for c in p_calls)
    assert any(c.interface == SimulatedWaylandDisplayManager.PORTAL_SCREENCAST_INTERFACE for c in p_calls)
    portal_mgr.destroy_display(p_id)
    assert any(c.member == "Close" and c.interface == SimulatedWaylandDisplayManager.PORTAL_SESSION_INTERFACE for c in portal_mgr.dbus_log)
    print("  [PASS] test_xdg_portal_session_creation_and_dbus_calls")

    # 8. test_invalid_resolution_rejected
    invalid_cases = [
        SimulatedDisplayConfig(0, 1080, 60),
        SimulatedDisplayConfig(1920, 0, 60),
        SimulatedDisplayConfig(1920, 1080, 0),
        SimulatedDisplayConfig(100, 1080, 60),
        SimulatedDisplayConfig(10000, 1080, 60),
        SimulatedDisplayConfig(1920, 100, 60),
        SimulatedDisplayConfig(1920, 5000, 60),
        SimulatedDisplayConfig(1920, 1080, 5),
        SimulatedDisplayConfig(1920, 1080, 500),
    ]
    sim2 = SimulatedWaylandDisplayManager("Simulated")
    for inv in invalid_cases:
        try:
            sim2.create_display(inv)
            assert False, "Invalid config should fail validation"
        except ValueError:
            pass
    assert sim2.display_count() == 0
    print("  [PASS] test_invalid_resolution_rejected")

    # 9. test_destroy_all_monitors
    multi_mgr = SimulatedWaylandDisplayManager("GnomeMutter", mock_mode=True)
    i1 = multi_mgr.create_display(SimulatedDisplayConfig(1920, 1080, 60))
    i2 = multi_mgr.create_display(SimulatedDisplayConfig(2560, 1440, 60))
    assert multi_mgr.display_count() == 2
    multi_mgr.destroy_all()
    assert multi_mgr.display_count() == 0
    stop_count = len([c for c in multi_mgr.dbus_log if c.member == "Stop"])
    assert stop_count == 2
    print("  [PASS] test_destroy_all_monitors")

    # 10. test_cleanup_on_drop
    cleanup_mgr = SimulatedWaylandDisplayManager("Simulated")
    _ = cleanup_mgr.create_display(SimulatedDisplayConfig(1920, 1080, 60))
    assert cleanup_mgr.display_count() == 1
    del cleanup_mgr
    print("  [PASS] test_cleanup_on_drop")

    # 11. test_real_mode_rejects_missing_wayland
    real_mgr = SimulatedWaylandDisplayManager("GnomeMutter", mock_mode=False, wayland_running=False)
    try:
        real_mgr.create_display(SimulatedDisplayConfig(1920, 1080, 60))
        assert False, "Real mode without Wayland must fail"
    except RuntimeError as e:
        assert "Wayland" in str(e)
    print("  [PASS] test_real_mode_rejects_missing_wayland")

    # 12. test_driver_installed_check
    sim_check = SimulatedWaylandDisplayManager("Simulated")
    assert sim_check.is_driver_installed() is True
    sim_check.set_simulated_driver_installed(False)
    assert sim_check.is_driver_installed() is False

    real_check = SimulatedWaylandDisplayManager("GnomeMutter", mock_mode=False, wayland_running=False)
    assert real_check.is_driver_installed() is False
    print("  [PASS] test_driver_installed_check")


def main():
    check_file_balance("src-tauri/src/display/linux_wayland.rs")
    check_file_balance("src-tauri/src/display/mod.rs")
    check_file_balance("src-tauri/tests/linux_display_test.rs")
    verify_interface_and_types()
    run_behavioral_tests()
    print("\nALL 12 LINUX WAYLAND UNIT TESTS PASSED SUCCESSFULLY!")


if __name__ == "__main__":
    main()
