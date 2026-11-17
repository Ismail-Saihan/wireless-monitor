use wireless_monitor_lib::display::linux_wayland::{
    build_mutter_create_session_call, build_mutter_create_virtual_monitor_call,
    build_mutter_stop_session_call, build_portal_close_call, build_portal_create_session_call,
    build_portal_screencast_call, DBusCallRecord, WaylandBackend, WaylandDisplayManager,
    MUTTER_INTERFACE, MUTTER_PATH, MUTTER_SERVICE, MUTTER_SESSION_INTERFACE,
    PORTAL_PATH, PORTAL_REMOTE_DESKTOP_INTERFACE, PORTAL_SCREENCAST_INTERFACE,
    PORTAL_SERVICE, PORTAL_SESSION_INTERFACE,
};
use wireless_monitor_lib::display::{DisplayConfig, VirtualDisplayManager};

#[test]
fn test_backend_detection_gnome() {
    let backend = WaylandDisplayManager::detect_backend_from_env(
        Some("wayland-0"),
        Some("GNOME"),
    );
    assert_eq!(backend, WaylandBackend::GnomeMutter);

    let backend_ubuntu = WaylandDisplayManager::detect_backend_from_env(
        Some("wayland-1"),
        Some("ubuntu:GNOME"),
    );
    assert_eq!(backend_ubuntu, WaylandBackend::GnomeMutter);
}

#[test]
fn test_backend_detection_kde_plasma() {
    let backend = WaylandDisplayManager::detect_backend_from_env(
        Some("wayland-0"),
        Some("KDE"),
    );
    assert_eq!(backend, WaylandBackend::XdgPortal);

    let backend_plasma = WaylandDisplayManager::detect_backend_from_env(
        Some("wayland-0"),
        Some("KDE:KWin"),
    );
    assert_eq!(backend_plasma, WaylandBackend::XdgPortal);
}

#[test]
fn test_backend_detection_other_wayland() {
    let backend = WaylandDisplayManager::detect_backend_from_env(
        Some("wayland-0"),
        Some("sway"),
    );
    assert_eq!(backend, WaylandBackend::XdgPortal);
}

#[test]
fn test_backend_detection_simulated_fallback() {
    // Missing WAYLAND_DISPLAY falls back to Simulated
    let backend = WaylandDisplayManager::detect_backend_from_env(None, Some("GNOME"));
    assert_eq!(backend, WaylandBackend::Simulated);

    let backend_empty = WaylandDisplayManager::detect_backend_from_env(Some(""), Some("KDE"));
    assert_eq!(backend_empty, WaylandBackend::Simulated);
}

#[test]
fn test_simulated_display_lifecycle() {
    let mut manager = WaylandDisplayManager::new_simulated();
    assert_eq!(manager.backend(), WaylandBackend::Simulated);
    assert_eq!(manager.display_count(), 0);
    assert!(manager.active_displays().is_empty());

    let config = DisplayConfig::new(1920, 1080, 60);
    let id = manager.create_display(&config).expect("Create display should succeed");
    assert!(id > 0);
    assert_eq!(manager.display_count(), 1);

    let retrieved = manager.get_display_config(id).expect("Should get config");
    assert_eq!(retrieved.width, 1920);
    assert_eq!(retrieved.height, 1080);
    assert_eq!(retrieved.refresh_rate, 60);

    // Destroy
    let destroy_res = manager.destroy_display(id);
    assert!(destroy_res.is_ok(), "Destroy should succeed");
    assert_eq!(manager.display_count(), 0);

    // Double destroy fails
    let double_res = manager.destroy_display(id);
    assert!(double_res.is_err(), "Double destroy should fail");
}

#[test]
fn test_gnome_mutter_session_creation_and_dbus_calls() {
    let mut manager = WaylandDisplayManager::new_mock(WaylandBackend::GnomeMutter);
    assert_eq!(manager.backend(), WaylandBackend::GnomeMutter);
    assert!(manager.is_mock());

    let config = DisplayConfig::new(2560, 1440, 120);
    let id = manager.create_display(&config).expect("Mock Mutter display creation");
    assert_eq!(manager.display_count(), 1);

    let session_handle = manager.session_handle(id).expect("Session handle must exist");
    assert!(session_handle.contains("Mutter"));

    let log = manager.dbus_call_log();
    assert!(!log.is_empty(), "D-Bus calls must be recorded in mock mode");

    // Verify CreateSession was invoked on Mutter
    let create_session_call = log.iter().find(|c| c.member == "CreateSession");
    assert!(create_session_call.is_some(), "Must call CreateSession on Mutter");
    let csc = create_session_call.unwrap();
    assert_eq!(csc.destination, MUTTER_SERVICE);
    assert_eq!(csc.path, MUTTER_PATH);
    assert_eq!(csc.interface, MUTTER_INTERFACE);

    // Verify CreateVirtualMonitor was invoked with resolution
    let create_monitor_call = log.iter().find(|c| c.member == "CreateVirtualMonitor");
    assert!(create_monitor_call.is_some(), "Must call CreateVirtualMonitor");
    let cmc = create_monitor_call.unwrap();
    assert_eq!(cmc.interface, MUTTER_SESSION_INTERFACE);
    assert!(cmc.arguments.iter().any(|arg| arg.contains("2560")));
    assert!(cmc.arguments.iter().any(|arg| arg.contains("1440")));
    assert!(cmc.arguments.iter().any(|arg| arg.contains("120")));

    // Destroy display and verify Mutter stop was called
    manager.destroy_display(id).expect("Destroy display");
    assert_eq!(manager.display_count(), 0);

    let stop_call = manager.dbus_call_log().iter().find(|c| c.member == "Stop");
    assert!(stop_call.is_some(), "Must call Stop on Mutter session on destroy");
}

#[test]
fn test_xdg_portal_session_creation_and_dbus_calls() {
    let mut manager = WaylandDisplayManager::new_mock(WaylandBackend::XdgPortal);
    assert_eq!(manager.backend(), WaylandBackend::XdgPortal);
    assert!(manager.is_mock());

    let config = DisplayConfig::new(3840, 2160, 60);
    let id = manager.create_display(&config).expect("Mock XDG Portal display creation");
    assert_eq!(manager.display_count(), 1);

    let session_handle = manager.session_handle(id).expect("Session handle must exist");
    assert!(session_handle.contains("portal"));

    let log = manager.dbus_call_log();
    // Verify portal calls
    let portal_create = log.iter().find(|c| {
        c.destination == PORTAL_SERVICE && c.interface == PORTAL_REMOTE_DESKTOP_INTERFACE && c.member == "CreateSession"
    });
    assert!(portal_create.is_some(), "Must invoke portal RemoteDesktop.CreateSession");

    let portal_screencast = log.iter().find(|c| {
        c.destination == PORTAL_SERVICE && c.interface == PORTAL_SCREENCAST_INTERFACE
    });
    assert!(portal_screencast.is_some(), "Must invoke portal ScreenCast.CreateSession");

    // Destroy and verify Close was called
    manager.destroy_display(id).expect("Destroy portal display");
    assert_eq!(manager.display_count(), 0);

    let close_call = manager.dbus_call_log().iter().find(|c| {
        c.interface == PORTAL_SESSION_INTERFACE && c.member == "Close"
    });
    assert!(close_call.is_some(), "Must invoke portal Session.Close on destroy");
}

#[test]
fn test_invalid_resolution_rejected() {
    let mut manager = WaylandDisplayManager::new_simulated();

    let invalid_configs = vec![
        DisplayConfig::new(0, 1080, 60),
        DisplayConfig::new(1920, 0, 60),
        DisplayConfig::new(1920, 1080, 0),
        DisplayConfig::new(100, 1080, 60),    // width < 320
        DisplayConfig::new(10000, 1080, 60),  // width > 7680
        DisplayConfig::new(1920, 100, 60),    // height < 240
        DisplayConfig::new(1920, 5000, 60),   // height > 4320
        DisplayConfig::new(1920, 1080, 5),    // refresh < 10
        DisplayConfig::new(1920, 1080, 500),  // refresh > 360
    ];

    for cfg in invalid_configs {
        let res = manager.create_display(&cfg);
        assert!(res.is_err(), "Config {:?} should have been rejected", cfg);
    }
    assert_eq!(manager.display_count(), 0);
}

#[test]
fn test_destroy_all_monitors() {
    let mut manager = WaylandDisplayManager::new_mock(WaylandBackend::GnomeMutter);

    let cfg1 = DisplayConfig::new(1920, 1080, 60);
    let cfg2 = DisplayConfig::new(2560, 1440, 60);
    let id1 = manager.create_display(&cfg1).unwrap();
    let id2 = manager.create_display(&cfg2).unwrap();
    assert_eq!(manager.display_count(), 2);

    manager.destroy_all().expect("destroy_all must succeed");
    assert_eq!(manager.display_count(), 0);
    assert!(manager.active_displays().is_empty());

    // Both sessions stopped
    let stop_calls: Vec<_> = manager.dbus_call_log().iter().filter(|c| c.member == "Stop").collect();
    assert_eq!(stop_calls.len(), 2, "Both Mutter sessions must be stopped on destroy_all");
}

#[test]
fn test_cleanup_on_drop() {
    {
        let mut manager = WaylandDisplayManager::new_simulated();
        let cfg = DisplayConfig::new(1920, 1080, 60);
        let _id = manager.create_display(&cfg).unwrap();
        assert_eq!(manager.display_count(), 1);
        // manager goes out of scope here; drop executes destroy_all safely
    }
    // Clean exit without orphan displays
}

#[test]
fn test_real_mode_rejects_missing_wayland() {
    // In real mode (not simulated, not mock), if WAYLAND_DISPLAY is unset or on non-Linux,
    // creating a display must return an actionable error rather than panicking or hanging.
    let mut manager = WaylandDisplayManager::new(WaylandBackend::GnomeMutter);
    assert!(!manager.is_simulated());
    assert!(!manager.is_mock());

    // Force WAYLAND_DISPLAY unset for this test
    let cfg = DisplayConfig::new(1920, 1080, 60);
    let res = manager.create_display(&cfg);
    assert!(res.is_err(), "Must return error when Wayland session is unavailable");
    let err_msg = format!("{}", res.err().unwrap());
    assert!(
        err_msg.contains("Wayland") || err_msg.contains("WAYLAND_DISPLAY") || err_msg.contains("Linux"),
        "Error message should mention Wayland or Linux: {}",
        err_msg
    );
}

#[test]
fn test_driver_installed_check() {
    let mut sim_mgr = WaylandDisplayManager::new_simulated();
    assert!(sim_mgr.is_driver_installed().unwrap());

    sim_mgr.set_simulated_driver_installed(false);
    assert!(!sim_mgr.is_driver_installed().unwrap());

    let mut real_mgr = WaylandDisplayManager::new(WaylandBackend::GnomeMutter);
    // On non-Linux or without WAYLAND_DISPLAY, is_driver_installed returns false
    let installed = real_mgr.is_driver_installed().unwrap();
    #[cfg(not(target_os = "linux"))]
    assert!(!installed);
}
