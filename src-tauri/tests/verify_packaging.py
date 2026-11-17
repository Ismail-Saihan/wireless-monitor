"""
Distribution Packaging & Distribution Bundling Test Suite.
Validates production packaging configuration in src-tauri/tauri.conf.json,
including Windows installer targets (NSIS, MSI) with bundled virtual display driver assets,
Linux distribution packages (.deb, AppImage) with system dependencies,
application metadata, and physical driver staging integrity.
"""

import os
import sys
import json


def check_file_balance(path):
    print(f"Checking delimiter balance in {path}...")
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
    print(f"  -> Delimiters balanced: {path}")


def resolve_paths():
    base_dir = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
    tauri_conf = os.path.join(base_dir, "src-tauri", "tauri.conf.json")
    driver_dir = os.path.join(base_dir, "drivers", "windows")
    return base_dir, tauri_conf, driver_dir


def test_tauri_conf_validity_and_identity(tauri_conf):
    print("Testing tauri.conf.json validity and app identity...")
    if not os.path.exists(tauri_conf):
        raise AssertionError(f"tauri.conf.json not found at {tauri_conf}")

    with open(tauri_conf, "r", encoding="utf-8") as f:
        conf = json.load(f)

    assert conf.get("productName") == "Wireless Monitor", f"Expected productName 'Wireless Monitor', got {conf.get('productName')}"
    assert conf.get("identifier") == "com.wirelessmonitor.app", f"Expected identifier 'com.wirelessmonitor.app', got {conf.get('identifier')}"
    assert conf.get("version"), "Version must be defined and non-empty"
    print("  [PASS] test_tauri_conf_validity_and_identity")
    return conf


def test_bundle_active_and_targets(conf):
    print("Testing bundle.active and distribution targets...")
    bundle = conf.get("bundle")
    assert isinstance(bundle, dict), "bundle field must be an object"
    assert bundle.get("active") is True, "bundle.active must be True"

    targets = bundle.get("targets")
    assert isinstance(targets, list), f"bundle.targets must be a list, got {type(targets)}"

    required_targets = ["nsis", "msi", "deb", "appimage"]
    for target in required_targets:
        assert target in targets, f"bundle.targets must contain '{target}', found: {targets}"

    print(f"  -> Validated targets: {targets}")
    print("  [PASS] test_bundle_active_and_targets")


def test_bundle_resources_driver_mapping(conf, base_dir):
    print("Testing bundle.resources driver mapping...")
    bundle = conf.get("bundle", {})
    resources = bundle.get("resources")
    assert isinstance(resources, list), f"bundle.resources must be a list, got {type(resources)}"
    assert len(resources) > 0, "bundle.resources must not be empty"

    # Verify that drivers/windows is referenced in resources
    has_driver_mapping = any("drivers/windows" in r or "drivers\\windows" in r for r in resources)
    assert has_driver_mapping, f"bundle.resources must include driver mapping (e.g. '../drivers/windows/**/*'), found: {resources}"

    print(f"  -> Validated resources mapping: {resources}")
    print("  [PASS] test_bundle_resources_driver_mapping")


def test_windows_packaging_config(conf):
    print("Testing Windows bundle configuration...")
    bundle = conf.get("bundle", {})
    windows = bundle.get("windows")
    assert isinstance(windows, dict), "bundle.windows must be configured"
    assert "nsis" in windows, "bundle.windows.nsis must be configured"
    print("  [PASS] test_windows_packaging_config")


def test_linux_packaging_config(conf):
    print("Testing Linux debian and appimage packaging configuration...")
    bundle = conf.get("bundle", {})
    linux = bundle.get("linux")
    assert isinstance(linux, dict), "bundle.linux must be configured"

    deb = linux.get("deb")
    assert isinstance(deb, dict), "bundle.linux.deb must be configured"

    depends = deb.get("depends")
    assert isinstance(depends, list), "bundle.linux.deb.depends must be a list"

    required_deps = ["libpipewire-0.3-0", "libdbus-1-3", "libxdo3"]
    for dep in required_deps:
        assert dep in depends, f"bundle.linux.deb.depends must contain '{dep}', found: {depends}"

    appimage = linux.get("appimage")
    assert isinstance(appimage, dict), "bundle.linux.appimage must be configured"
    print(f"  -> Validated Debian dependencies: {depends}")
    print("  [PASS] test_linux_packaging_config")


def test_metadata_properties(conf):
    print("Testing application metadata properties...")
    bundle = conf.get("bundle", {})

    category = bundle.get("category")
    assert category == "Utility", f"bundle.category must be 'Utility', got '{category}'"

    copyright_val = bundle.get("copyright")
    assert copyright_val, "bundle.copyright must be present and non-empty"

    desc = bundle.get("description") or bundle.get("shortDescription") or conf.get("description")
    assert desc, "Application description must be present and non-empty"

    long_desc = bundle.get("longDescription")
    assert long_desc, "bundle.longDescription must be present and non-empty"

    print(f"  -> Category: {category}")
    print(f"  -> Copyright: {copyright_val}")
    print(f"  -> Description: {desc}")
    print(f"  -> Long Description: {long_desc}")
    print("  [PASS] test_metadata_properties")


def test_driver_assets_existence(driver_dir):
    print("Testing actual staged driver assets to be bundled...")
    assert os.path.isdir(driver_dir), f"Driver directory not found at {driver_dir}"

    required_files = [
        "VirtualDisplayDriver.inf",
        "VirtualDisplayDriver.cer",
        "vdd.exe",
        "install_driver.bat",
        "uninstall_driver.bat",
        os.path.join("x64", "VirtualDisplayDriver.sys"),
        os.path.join("x64", "VirtualDisplayDriver.dll"),
    ]

    for rel in required_files:
        full = os.path.join(driver_dir, rel)
        assert os.path.isfile(full), f"Required driver asset missing: {full}"
        size = os.path.getsize(full)
        assert size > 0, f"Driver asset is empty: {full} ({size} bytes)"
        print(f"  -> Verified asset: {rel} ({size} bytes)")

    print("  [PASS] test_driver_assets_existence")


def main():
    print("=" * 70)
    print("Starting Packaging Configuration & Distribution Bundling Verification")
    print("=" * 70)

    base_dir, tauri_conf, driver_dir = resolve_paths()
    rs_test_path = os.path.join(base_dir, "src-tauri", "tests", "packaging_test.rs")

    check_file_balance(rs_test_path)

    conf = test_tauri_conf_validity_and_identity(tauri_conf)
    test_bundle_active_and_targets(conf)
    test_bundle_resources_driver_mapping(conf, base_dir)
    test_windows_packaging_config(conf)
    test_linux_packaging_config(conf)
    test_metadata_properties(conf)
    test_driver_assets_existence(driver_dir)

    print("=" * 70)
    print("ALL 7 PACKAGING VERIFICATION CHECKS PASSED SUCCESSFULLY!")
    print("=" * 70)


if __name__ == "__main__":
    try:
        main()
    except Exception as e:
        print(f"\n[FAIL] Verification error: {e}", file=sys.stderr)
        sys.exit(1)
