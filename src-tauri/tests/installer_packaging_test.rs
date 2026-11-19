//! Unit and integration tests for installer packaging and runtime resource resolution.

use std::path::{Path, PathBuf};
use wireless_monitor_lib::display::WindowsIddCxManager;

#[test]
fn test_driver_script_name_resolution() {
    let script_name = "install_driver.bat";
    let manager = WindowsIddCxManager::new();
    let resolved = manager.resolve_script_path(Path::new(script_name));
    assert!(
        resolved.is_some(),
        "WindowsIddCxManager::resolve_script_path must locate {:?} from repository or runtime environment",
        script_name
    );
    let resolved_path = resolved.unwrap();
    assert!(
        resolved_path.exists(),
        "Resolved driver installation script must actually exist on disk: {:?}",
        resolved_path
    );
}

#[test]
fn test_icon_assets_presence_and_structure() {
    let required_icons = [
        "icons/icon.ico",
        "icons/icon.png",
        "icons/32x32.png",
        "icons/128x128.png",
        "icons/128x128@2x.png",
    ];

    let base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for icon in &required_icons {
        let full_path = base_dir.join(icon);
        assert!(
            full_path.exists(),
            "Required application icon {:?} must exist in src-tauri/",
            icon
        );
        let metadata = std::fs::metadata(&full_path).expect("Failed to read metadata");
        assert!(metadata.len() > 0, "Icon file {:?} must not be empty", icon);
    }
}

#[test]
fn test_nsis_installer_hooks_presence_and_structure() {
    let base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let hooks_path = base_dir.join("nsis/installerHooks.nsh");
    assert!(
        hooks_path.exists(),
        "NSIS installer hooks script must exist at {:?}",
        hooks_path
    );
    let content = std::fs::read_to_string(&hooks_path).expect("Failed to read installerHooks.nsh");
    assert!(
        content.contains("NSIS_HOOK_POSTINSTALL"),
        "installerHooks.nsh must define NSIS_HOOK_POSTINSTALL"
    );
    assert!(
        content.contains("NSIS_HOOK_PREUNINSTALL"),
        "installerHooks.nsh must define NSIS_HOOK_PREUNINSTALL"
    );
    assert!(
        content.contains("nsExec::ExecToLog"),
        "installerHooks.nsh must use nsExec::ExecToLog for silent execution"
    );
    assert!(
        content.contains("install_driver.bat"),
        "installerHooks.nsh must reference install_driver.bat"
    );
    assert!(
        content.contains("uninstall_driver.bat"),
        "installerHooks.nsh must reference uninstall_driver.bat"
    );
    assert!(
        content.contains("Wireless Monitor"),
        "installerHooks.nsh must configure firewall rule for Wireless Monitor"
    );
}
