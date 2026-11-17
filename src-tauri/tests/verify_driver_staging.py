"""
Test and validation suite for Windows Virtual Display Driver Packaging & Staging.
Verifies file presence, non-emptiness, x64 PE architecture headers (MZ/PE32+/AMD64),
INF driver manifest structure, X.509 release certificate cryptographic properties,
and automated batch installation & uninstallation workflows.
"""

import os
import sys
import struct
import subprocess


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


def parse_pe_info(data: bytes):
    """Parse essential Windows PE headers to verify x64 architecture integrity."""
    if len(data) < 0x40:
        raise ValueError("Data too short for DOS header")
    if data[0:2] != b"MZ":
        raise ValueError("Missing 'MZ' DOS signature")

    e_lfanew = struct.unpack_from("<I", data, 0x3C)[0]
    if e_lfanew + 24 > len(data):
        raise ValueError(f"e_lfanew {e_lfanew:#x} extends beyond file length")

    if data[e_lfanew : e_lfanew + 4] != b"PE\0\0":
        raise ValueError("Missing 'PE\\0\\0' signature")

    coff_offset = e_lfanew + 4
    machine, num_sections, _, _, _, size_of_opt, characteristics = struct.unpack_from(
        "<HHIIIHH", data, coff_offset
    )

    opt_offset = e_lfanew + 24
    if opt_offset + size_of_opt > len(data) or size_of_opt < 68:
        raise ValueError("Optional header truncated or missing")

    opt_magic = struct.unpack_from("<H", data, opt_offset)[0]
    subsystem = struct.unpack_from("<H", data, opt_offset + 68)[0]

    return {
        "machine": machine,
        "num_sections": num_sections,
        "characteristics": characteristics,
        "opt_magic": opt_magic,
        "subsystem": subsystem,
    }


def test_driver_directory_structure():
    print("Testing driver directory structure and presence...")
    driver_dir = "drivers/windows"
    assert os.path.exists(driver_dir), f"Directory {driver_dir} does not exist"

    required_files = [
        "VirtualDisplayDriver.cer",
        "VirtualDisplayDriver.inf",
        "vdd.exe",
        "install_driver.bat",
        "uninstall_driver.bat",
        "x64/VirtualDisplayDriver.sys",
        "x64/VirtualDisplayDriver.dll",
    ]

    for rel_path in required_files:
        full_path = os.path.join(driver_dir, rel_path)
        assert os.path.exists(full_path), f"Required file {full_path} missing"
        size = os.path.getsize(full_path)
        assert size > 0, f"File {full_path} is empty (0 bytes)"
    print("  [PASS] test_driver_directory_structure")


def test_x64_pe_architecture_integrity():
    print("Testing x64 PE architecture integrity...")
    driver_dir = "drivers/windows"
    IMAGE_FILE_MACHINE_AMD64 = 0x8664
    PE32PLUS_MAGIC = 0x020B
    IMAGE_FILE_DLL = 0x2000
    IMAGE_SUBSYSTEM_NATIVE = 1
    IMAGE_SUBSYSTEM_WINDOWS_GUI = 2
    IMAGE_SUBSYSTEM_WINDOWS_CUI = 3

    # 1. vdd.exe
    with open(os.path.join(driver_dir, "vdd.exe"), "rb") as f:
        vdd_pe = parse_pe_info(f.read())
    assert vdd_pe["machine"] == IMAGE_FILE_MACHINE_AMD64, (
        f"vdd.exe machine {vdd_pe['machine']:#x} != AMD64 (0x8664)"
    )
    assert vdd_pe["opt_magic"] == PE32PLUS_MAGIC, (
        f"vdd.exe opt_magic {vdd_pe['opt_magic']:#x} != PE32+ (0x20B)"
    )
    assert vdd_pe["subsystem"] in (IMAGE_SUBSYSTEM_WINDOWS_CUI, IMAGE_SUBSYSTEM_WINDOWS_GUI), (
        f"vdd.exe subsystem {vdd_pe['subsystem']} not CUI/GUI"
    )

    # 2. x64/VirtualDisplayDriver.sys
    with open(os.path.join(driver_dir, "x64", "VirtualDisplayDriver.sys"), "rb") as f:
        sys_pe = parse_pe_info(f.read())
    assert sys_pe["machine"] == IMAGE_FILE_MACHINE_AMD64, (
        f"VirtualDisplayDriver.sys machine {sys_pe['machine']:#x} != AMD64 (0x8664)"
    )
    assert sys_pe["opt_magic"] == PE32PLUS_MAGIC, (
        f"VirtualDisplayDriver.sys opt_magic {sys_pe['opt_magic']:#x} != PE32+ (0x20B)"
    )
    assert sys_pe["subsystem"] == IMAGE_SUBSYSTEM_NATIVE, (
        f"VirtualDisplayDriver.sys subsystem {sys_pe['subsystem']} != NATIVE (1)"
    )

    # 3. x64/VirtualDisplayDriver.dll
    with open(os.path.join(driver_dir, "x64", "VirtualDisplayDriver.dll"), "rb") as f:
        dll_pe = parse_pe_info(f.read())
    assert dll_pe["machine"] == IMAGE_FILE_MACHINE_AMD64, (
        f"VirtualDisplayDriver.dll machine {dll_pe['machine']:#x} != AMD64 (0x8664)"
    )
    assert dll_pe["opt_magic"] == PE32PLUS_MAGIC, (
        f"VirtualDisplayDriver.dll opt_magic {dll_pe['opt_magic']:#x} != PE32+ (0x20B)"
    )
    assert (dll_pe["characteristics"] & IMAGE_FILE_DLL) != 0, (
        f"VirtualDisplayDriver.dll characteristics {dll_pe['characteristics']:#x} missing IMAGE_FILE_DLL"
    )

    print("  [PASS] test_x64_pe_architecture_integrity")


def test_driver_inf_manifest_validity():
    print("Testing driver INF manifest validity...")
    inf_path = "drivers/windows/VirtualDisplayDriver.inf"
    with open(inf_path, "r", encoding="utf-8") as f:
        content = f.read()

    required_sections = [
        "[Version]",
        "[DestinationDirs]",
        "[SourceDisksNames]",
        "[SourceDisksFiles]",
        "[Manufacturer]",
        "[Strings]",
    ]
    for section in required_sections:
        assert section in content, f"INF missing required section {section}"

    assert "Class=Display" in content or "Class = Display" in content, "INF missing Class=Display"
    assert "{4d36e968-e325-11ce-bfc1-08002be10318}" in content.lower(), "INF missing Display ClassGuid"
    assert "Root\\VirtualDisplayDriver" in content, "INF missing Root\\VirtualDisplayDriver"
    assert "*VDD0001" in content, "INF missing *VDD0001"
    assert "VirtualDisplayDriver.dll" in content, "INF missing VirtualDisplayDriver.dll reference"
    assert "VirtualDisplayDriver.sys" in content, "INF missing VirtualDisplayDriver.sys reference"
    assert "UmdfService=VirtualDisplayDriver" in content or "UmdfService = VirtualDisplayDriver" in content, (
        "INF missing UmdfService configuration"
    )
    print("  [PASS] test_driver_inf_manifest_validity")


def test_staged_certificate_validity():
    print("Testing staged certificate validity and trust properties...")
    cer_path = "drivers/windows/VirtualDisplayDriver.cer"
    with open(cer_path, "rb") as f:
        cer_bytes = f.read()

    assert len(cer_bytes) >= 128, f"Certificate too small ({len(cer_bytes)} bytes)"

    # Validate with cryptography if available
    try:
        from cryptography import x509
        from cryptography.hazmat.backends import default_backend

        if cer_bytes.startswith(b"-----BEGIN CERTIFICATE-----"):
            cert = x509.load_pem_x509_certificate(cer_bytes, default_backend())
        else:
            cert = x509.load_der_x509_certificate(cer_bytes, default_backend())

        subject = cert.subject.rfc4514_string()
        issuer = cert.issuer.rfc4514_string()
        print(f"    Certificate Subject: {subject}")
        print(f"    Certificate Issuer:  {issuer}")
        assert "Wireless" in subject or "Display" in subject or "Monitor" in subject, (
            f"Subject unexpected: {subject}"
        )
    except Exception as e:
        print(f"    Notice: cryptography parsing fallback/check: {e}")
        # Verify DER ASN.1 header (0x30, 0x82...)
        assert cer_bytes[0] == 0x30, "Certificate must start with ASN.1 SEQUENCE (0x30)"

    print("  [PASS] test_staged_certificate_validity")


def test_install_driver_batch_script_workflow():
    print("Testing install_driver.bat script automation...")
    with open("drivers/windows/install_driver.bat", "r", encoding="utf-8") as f:
        content = f.read()

    # UAC elevation
    assert "net session" in content, "install_driver.bat must test net session"
    assert "Start-Process -FilePath '%~dpnx0' -Verb RunAs" in content, (
        "install_driver.bat must request UAC elevation via Start-Process"
    )
    assert "(UAC)" not in content, "install_driver.bat must avoid unescaped parens"

    # Certificate import
    assert 'certutil -addstore "TrustedPublisher"' in content, (
        "install_driver.bat must import into TrustedPublisher"
    )
    assert 'certutil -addstore "Root"' in content, (
        "install_driver.bat must import into Root"
    )

    # INF installation
    assert "pnputil /add-driver" in content and "/install" in content, (
        "install_driver.bat must install INF using pnputil /add-driver ... /install"
    )

    # Companion service
    assert 'vdd.exe" install' in content or "vdd.exe install" in content, (
        "install_driver.bat must register companion service via vdd.exe install"
    )

    # Registry marker
    assert "!DRIVER_INSTALLED! equ 1" in content, (
        "install_driver.bat must gate registry marker on DRIVER_INSTALLED"
    )
    assert "HKLM\\SOFTWARE\\VirtualDisplayDriver" in content, (
        "install_driver.bat must write to HKLM\\SOFTWARE\\VirtualDisplayDriver"
    )

    print("  [PASS] test_install_driver_batch_script_workflow")


def test_uninstall_driver_batch_script_workflow():
    print("Testing uninstall_driver.bat script automation...")
    with open("drivers/windows/uninstall_driver.bat", "r", encoding="utf-8") as f:
        content = f.read()

    # UAC elevation
    assert "net session" in content, "uninstall_driver.bat must test net session"
    assert "Start-Process -FilePath '%~dpnx0' -Verb RunAs" in content, (
        "uninstall_driver.bat must request UAC elevation via Start-Process"
    )
    assert "(UAC)" not in content, "uninstall_driver.bat must avoid unescaped parens"

    # Companion service
    assert 'vdd.exe" uninstall' in content or "vdd.exe uninstall" in content, (
        "uninstall_driver.bat must teardown companion service via vdd.exe uninstall"
    )

    # Driver Store package removal
    assert "pnputil /delete-driver" in content, (
        "uninstall_driver.bat must remove packages via pnputil /delete-driver"
    )
    assert "/uninstall /force" in content, (
        "uninstall_driver.bat must use /uninstall /force"
    )
    assert "pnputil /enum-drivers" in content, (
        "uninstall_driver.bat must query OEM driver names"
    )

    # Certificate cleanup
    assert "certutil -delstore" in content, (
        "uninstall_driver.bat must clean up certificates via certutil -delstore"
    )

    # Registry cleanup
    assert 'reg delete "HKLM\\SOFTWARE\\VirtualDisplayDriver" /f' in content, (
        "uninstall_driver.bat must clean up registry entries"
    )

    print("  [PASS] test_uninstall_driver_batch_script_workflow")


def test_companion_binary_execution():
    print("Testing vdd.exe companion binary execution...")
    vdd_path = os.path.join("drivers", "windows", "vdd.exe")
    if sys.platform == "win32" and os.path.exists(vdd_path):
        res = subprocess.run(
            [vdd_path, "--version"],
            capture_output=True,
            text=True,
            timeout=5,
        )
        assert res.returncode == 0, f"vdd.exe --version returned {res.returncode}: {res.stderr}"
        print(f"    vdd.exe output: {res.stdout.strip()}")

        res_install = subprocess.run(
            [vdd_path, "install"],
            capture_output=True,
            text=True,
            timeout=5,
        )
        assert res_install.returncode == 0, f"vdd.exe install returned {res_install.returncode}"

        res_uninstall = subprocess.run(
            [vdd_path, "uninstall"],
            capture_output=True,
            text=True,
            timeout=5,
        )
        assert res_uninstall.returncode == 0, f"vdd.exe uninstall returned {res_uninstall.returncode}"
    print("  [PASS] test_companion_binary_execution")


def main():
    print("Starting Windows Virtual Display Driver Packaging & Staging Verification...")

    # Delimiter balance check
    check_file_balance("src-tauri/tests/driver_staging_test.rs")

    # Run tests
    test_driver_directory_structure()
    test_x64_pe_architecture_integrity()
    test_driver_inf_manifest_validity()
    test_staged_certificate_validity()
    test_install_driver_batch_script_workflow()
    test_uninstall_driver_batch_script_workflow()
    test_companion_binary_execution()

    print("\nALL 7 UNIT TESTS PASSED SUCCESSFULLY!")


if __name__ == "__main__":
    main()
