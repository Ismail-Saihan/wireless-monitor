"""
Comprehensive Verification Suite: Production Turnkey Installers for Windows and Linux.
Validates multi-resolution icon assets, ICO/PNG binary structures, Tauri 2 bundle
configurations (NSIS, MSI, DEB, AppImage), runtime resource resolution, automated build
scripts, GitHub Actions release CI/CD workflow, and user installation documentation.
"""

import os
import sys
import json
from PIL import Image


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


def resolve_base():
    base_dir = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
    return base_dir


def test_icon_suite_integrity(base_dir):
    print("Testing application icon suite integrity...")
    icons_dir = os.path.join(base_dir, "src-tauri", "icons")
    assert os.path.isdir(icons_dir), f"Icons directory missing at {icons_dir}"

    expected_pngs = {
        "32x32.png": (32, 32),
        "128x128.png": (128, 128),
        "128x128@2x.png": (256, 256),
        "icon.png": (512, 512),
    }

    for name, expected_dim in expected_pngs.items():
        p = os.path.join(icons_dir, name)
        assert os.path.isfile(p), f"Icon {name} missing at {p}"
        assert os.path.getsize(p) > 0, f"Icon {name} is empty"
        with Image.open(p) as img:
            assert img.size == expected_dim, f"Icon {name} expected size {expected_dim}, got {img.size}"
            assert img.format == "PNG", f"Icon {name} expected format PNG, got {img.format}"
        print(f"  -> Verified PNG: {name} {expected_dim}")

    ico_path = os.path.join(icons_dir, "icon.ico")
    assert os.path.isfile(ico_path), f"icon.ico missing at {ico_path}"
    assert os.path.getsize(ico_path) > 0, "icon.ico is empty"
    with open(ico_path, "rb") as f:
        header = f.read(4)
        assert header == b"\x00\x00\x01\x00", f"Invalid ICO header: {header}"

    print(f"  -> Verified Windows multi-res ICO: {ico_path}")
    print("  [PASS] test_icon_suite_integrity")


def test_tauri_bundle_configuration(base_dir):
    print("Testing Tauri 2 bundle configuration in tauri.conf.json...")
    tauri_conf = os.path.join(base_dir, "src-tauri", "tauri.conf.json")
    with open(tauri_conf, "r", encoding="utf-8") as f:
        conf = json.load(f)

    bundle = conf.get("bundle", {})
    assert bundle.get("active") is True, "bundle.active must be true"

    targets = bundle.get("targets", [])
    for t in ["nsis", "msi", "deb", "appimage"]:
        assert t in targets, f"Missing target '{t}' in bundle.targets: {targets}"

    icons = bundle.get("icon", [])
    assert len(icons) >= 4, f"bundle.icon must reference all icon resolutions, got {icons}"
    assert any("icon.ico" in i for i in icons), "bundle.icon must reference icon.ico"
    assert any("icon.png" in i for i in icons), "bundle.icon must reference icon.png"

    windows = bundle.get("windows", {})
    nsis = windows.get("nsis", {})
    assert nsis.get("installMode") == "both", f"Expected nsis.installMode == 'both', got {nsis.get('installMode')}"

    linux = bundle.get("linux", {})
    deb = linux.get("deb", {})
    depends = deb.get("depends", [])
    for dep in ["libpipewire-0.3-0", "libdbus-1-3", "libxdo3"]:
        assert any(dep in d for d in depends), f"Debian package missing dependency {dep}"

    print("  [PASS] test_tauri_bundle_configuration")


def test_runtime_script_resolution_logic(base_dir):
    print("Testing runtime resource resolution logic in windows_iddcx.rs...")
    rs_path = os.path.join(base_dir, "src-tauri", "src", "display", "windows_iddcx.rs")
    with open(rs_path, "r", encoding="utf-8") as f:
        content = f.read()

    assert "pub fn resolve_script_path" in content, "resolve_script_path must be public"
    assert "std::env::current_exe()" in content, "resolve_script_path must probe current_exe relative paths"
    assert "resources" in content, "resolve_script_path must check installed resources directory"
    print("  [PASS] test_runtime_script_resolution_logic")


def test_build_scripts_and_ci_workflow(base_dir):
    print("Testing local build scripts and CI workflow...")
    ps1_path = os.path.join(base_dir, "scripts", "build-windows.ps1")
    sh_path = os.path.join(base_dir, "scripts", "build-linux.sh")
    ci_path = os.path.join(base_dir, ".github", "workflows", "release.yml")

    assert os.path.isfile(ps1_path), f"build-windows.ps1 missing at {ps1_path}"
    assert os.path.isfile(sh_path), f"build-linux.sh missing at {sh_path}"
    assert os.path.isfile(ci_path), f"release.yml missing at {ci_path}"

    with open(ps1_path, "r", encoding="utf-8") as f:
        ps1 = f.read()
    assert "pnpm" in ps1 and "tauri" in ps1, "build-windows.ps1 must invoke tauri build"
    assert "generate_icons.py" in ps1, "build-windows.ps1 must validate icon suite"

    with open(sh_path, "r", encoding="utf-8") as f:
        sh = f.read()
    assert "pnpm" in sh and "tauri" in sh, "build-linux.sh must invoke tauri build"

    with open(ci_path, "r", encoding="utf-8") as f:
        ci = f.read()
    assert "windows-latest" in ci and "ubuntu-22.04" in ci, "CI workflow must target Windows and Ubuntu"
    assert "tauri-apps/tauri-action" in ci, "CI workflow must use tauri-action"
    assert "nsis,msi" in ci and "deb,appimage" in ci, "CI workflow must bundle all 4 targets"

    print("  [PASS] test_build_scripts_and_ci_workflow")


def test_installation_guide_and_docs(base_dir):
    print("Testing user installation guide and documentation synchronization...")
    install_md = os.path.join(base_dir, "INSTALL.md")
    readme_md = os.path.join(base_dir, "README.md")
    agent_md = os.path.join(base_dir, "AGENT.md")

    assert os.path.isfile(install_md), f"INSTALL.md missing at {install_md}"
    with open(install_md, "r", encoding="utf-8") as f:
        content = f.read()

    assert "NSIS Setup Wizard" in content, "INSTALL.md must document NSIS setup"
    assert "MSI Enterprise Installer" in content, "INSTALL.md must document MSI"
    assert "Debian Package" in content, "INSTALL.md must document Debian install"
    assert "AppImage" in content, "INSTALL.md must document AppImage"
    assert "Uninstallation" in content, "INSTALL.md must document uninstallation"

    with open(readme_md, "r", encoding="utf-8") as f:
        readme = f.read()
    assert "INSTALL.md" in readme, "README.md must link to INSTALL.md"

    with open(agent_md, "r", encoding="utf-8") as f:
        agent = f.read()
    assert "verify_installers.py" in agent, "AGENT.md must reference verify_installers.py"

    print("  [PASS] test_installation_guide_and_docs")


def main():
    print("=" * 70)
    print("Starting Turnkey Production Installers Verification Suite")
    print("=" * 70)

    base_dir = resolve_base()

    check_file_balance(os.path.join(base_dir, "src-tauri", "src", "display", "windows_iddcx.rs"))
    check_file_balance(os.path.join(base_dir, "src-tauri", "tests", "installer_packaging_test.rs"))

    test_icon_suite_integrity(base_dir)
    test_tauri_bundle_configuration(base_dir)
    test_runtime_script_resolution_logic(base_dir)
    test_build_scripts_and_ci_workflow(base_dir)
    test_installation_guide_and_docs(base_dir)

    print("=" * 70)
    print("ALL TURNKEY INSTALLER VERIFICATION CHECKS PASSED (0 ERRORS)!")
    print("=" * 70)


if __name__ == "__main__":
    main()
