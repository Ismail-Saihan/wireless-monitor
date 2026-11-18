//! Distribution Packaging and Bundling Test Suite
//!
//! Validates the Tauri 2.0 application packaging configuration in `tauri.conf.json`,
//! ensuring production-ready distribution targets (NSIS & MSI for Windows, DEB & AppImage for Linux),
//! resource bundling of Windows virtual display driver artifacts (`drivers/windows/**/*`),
//! Linux runtime dependencies (`libpipewire-0.3-0`, `libdbus-1-3`, `libxdo3`),
//! and application metadata (category, descriptions, copyright).

use std::fs;
use std::path::PathBuf;

/// Helper to locate tauri.conf.json from various test execution roots
fn get_tauri_conf_path() -> PathBuf {
    let candidates = [
        PathBuf::from("src-tauri/tauri.conf.json"),
        PathBuf::from("tauri.conf.json"),
        PathBuf::from("../tauri.conf.json"),
        PathBuf::from("../../src-tauri/tauri.conf.json"),
    ];

    for candidate in &candidates {
        if candidate.exists() && candidate.is_file() {
            return candidate.clone();
        }
    }

    PathBuf::from("src-tauri/tauri.conf.json")
}

/// Helper to locate workspace root directory
fn get_workspace_root() -> PathBuf {
    let candidates = [
        PathBuf::from("."),
        PathBuf::from(".."),
        PathBuf::from("../.."),
    ];

    for candidate in &candidates {
        if candidate.join("drivers/windows").is_dir() && candidate.join("src-tauri").is_dir() {
            return candidate.clone();
        }
    }

    PathBuf::from(".")
}

/// Helper to load and parse tauri.conf.json as JSON Value
fn load_tauri_conf() -> serde_json::Value {
    let conf_path = get_tauri_conf_path();
    assert!(
        conf_path.exists(),
        "tauri.conf.json must exist at path: {:?}",
        conf_path
    );

    let content = fs::read_to_string(&conf_path)
        .unwrap_or_else(|e| panic!("Failed to read tauri.conf.json from {:?}: {}", conf_path, e));

    serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("tauri.conf.json contains invalid JSON: {}", e))
}

#[test]
fn test_tauri_conf_json_validity_and_identity() {
    let conf = load_tauri_conf();

    let product_name = conf
        .get("productName")
        .and_then(|v| v.as_str())
        .expect("tauri.conf.json must define a 'productName' string");
    assert_eq!(product_name, "Wireless Monitor");

    let version = conf
        .get("version")
        .and_then(|v| v.as_str())
        .expect("tauri.conf.json must define a 'version' string");
    assert!(!version.is_empty(), "Version must not be empty");

    let identifier = conf
        .get("identifier")
        .and_then(|v| v.as_str())
        .expect("tauri.conf.json must define an 'identifier' string");
    assert_eq!(identifier, "com.wirelessmonitor.app");
}

#[test]
fn test_bundle_active_and_targets() {
    let conf = load_tauri_conf();

    let bundle = conf
        .get("bundle")
        .and_then(|v| v.as_object())
        .expect("tauri.conf.json must define a 'bundle' object");

    let active = bundle
        .get("active")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    assert!(active, "'bundle.active' must be true");

    let targets = bundle
        .get("targets")
        .and_then(|v| v.as_array())
        .expect("'bundle.targets' must be an array of target package formats");

    let target_strings: Vec<&str> = targets
        .iter()
        .filter_map(|t| t.as_str())
        .collect();

    let required_targets = ["nsis", "msi", "deb", "appimage"];
    for required in &required_targets {
        assert!(
            target_strings.contains(required),
            "bundle.targets must contain '{}', found targets: {:?}",
            required,
            target_strings
        );
    }
}

#[test]
fn test_bundle_resources_driver_mapping() {
    let conf = load_tauri_conf();

    let bundle = conf
        .get("bundle")
        .and_then(|v| v.as_object())
        .expect("tauri.conf.json must define a 'bundle' object");

    let resources = bundle
        .get("resources")
        .and_then(|v| v.as_array())
        .expect("'bundle.resources' must be an array mapping bundled files");

    let resource_strings: Vec<&str> = resources
        .iter()
        .filter_map(|r| r.as_str())
        .collect();

    let has_driver_resource = resource_strings
        .iter()
        .any(|r| r.contains("drivers/windows") || r.contains("drivers\\windows"));

    assert!(
        has_driver_resource,
        "'bundle.resources' must include drivers/windows mapping (e.g. '../drivers/windows/**/*'), found: {:?}",
        resource_strings
    );
}

#[test]
fn test_windows_packaging_configuration() {
    let conf = load_tauri_conf();

    let bundle = conf
        .get("bundle")
        .and_then(|v| v.as_object())
        .expect("tauri.conf.json must define a 'bundle' object");

    let windows = bundle
        .get("windows")
        .and_then(|v| v.as_object())
        .expect("'bundle.windows' must be configured for Windows installer packaging");

    assert!(
        windows.contains_key("nsis"),
        "'bundle.windows' must configure 'nsis' installer options"
    );
}

#[test]
fn test_linux_deb_and_appimage_packaging_configuration() {
    let conf = load_tauri_conf();

    let bundle = conf
        .get("bundle")
        .and_then(|v| v.as_object())
        .expect("tauri.conf.json must define a 'bundle' object");

    let linux = bundle
        .get("linux")
        .and_then(|v| v.as_object())
        .expect("'bundle.linux' must be configured for Linux package distribution");

    let deb = linux
        .get("deb")
        .and_then(|v| v.as_object())
        .expect("'bundle.linux.deb' must configure Debian package settings");

    let depends = deb
        .get("depends")
        .and_then(|v| v.as_array())
        .expect("'bundle.linux.deb.depends' must be an array of package dependencies");

    let dep_strings: Vec<&str> = depends
        .iter()
        .filter_map(|d| d.as_str())
        .collect();

    let required_deps = ["libpipewire-0.3-0", "libdbus-1-3", "libxdo3"];
    for req_dep in &required_deps {
        assert!(
            dep_strings.contains(req_dep),
            "Debian dependencies must include '{}', found: {:?}",
            req_dep,
            dep_strings
        );
    }

    assert!(
        linux.contains_key("appimage"),
        "'bundle.linux' must configure 'appimage' packaging settings"
    );
}

#[test]
fn test_application_metadata_properties() {
    let conf = load_tauri_conf();

    let bundle = conf
        .get("bundle")
        .and_then(|v| v.as_object())
        .expect("tauri.conf.json must define a 'bundle' object");

    let category = bundle
        .get("category")
        .and_then(|v| v.as_str())
        .expect("'bundle.category' must be specified");
    assert_eq!(category, "Utility");

    let copyright = bundle
        .get("copyright")
        .and_then(|v| v.as_str())
        .expect("'bundle.copyright' must be specified");
    assert!(!copyright.is_empty(), "Copyright must not be empty");

    let description_present = bundle.contains_key("description")
        || bundle.contains_key("shortDescription")
        || conf.get("description").is_some();
    assert!(
        description_present,
        "Package description must be present in bundle configuration"
    );

    let long_description_present = bundle.contains_key("longDescription");
    assert!(
        long_description_present,
        "Package longDescription must be present in bundle configuration"
    );
}

#[test]
fn test_bundled_driver_files_exist_and_non_empty() {
    let root = get_workspace_root();
    let driver_dir = root.join("drivers/windows");

    assert!(
        driver_dir.is_dir(),
        "Driver directory {:?} must exist in workspace",
        driver_dir
    );

    let required_files = [
        "VirtualDisplayDriver.inf",
        "VirtualDisplayDriver.cer",
        "vdd.exe",
        "install_driver.bat",
        "uninstall_driver.bat",
        "x64/VirtualDisplayDriver.sys",
        "x64/VirtualDisplayDriver.dll",
    ];

    for rel_path in &required_files {
        let file_path = driver_dir.join(rel_path);
        assert!(
            file_path.is_file(),
            "Required bundled driver file {:?} must exist",
            file_path
        );

        let metadata = fs::metadata(&file_path)
            .unwrap_or_else(|e| panic!("Failed to read metadata for {:?}: {}", file_path, e));
        assert!(
            metadata.len() > 0,
            "Bundled driver file {:?} must not be empty (size: {} bytes)",
            file_path,
            metadata.len()
        );
    }
}
