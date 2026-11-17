//! Driver Packaging and Staging Test Suite
//!
//! Validates the staged Windows Virtual Display Driver release artifacts,
//! x64 PE architecture headers, INF installation manifests, security certificates,
//! and automation batch installation/uninstallation scripts.

use std::fs;
use std::path::{Path, PathBuf};

/// Helper to resolve driver directory from workspace root or test runner location
fn get_driver_dir() -> PathBuf {
    let candidates = [
        PathBuf::from("drivers/windows"),
        PathBuf::from("../drivers/windows"),
        PathBuf::from("../../drivers/windows"),
    ];

    for candidate in &candidates {
        if candidate.exists() && candidate.is_dir() {
            return candidate.clone();
        }
    }

    PathBuf::from("drivers/windows")
}

/// Helper struct for parsed Windows PE header information
#[derive(Debug)]
struct PeHeaderInfo {
    machine: u16,
    number_of_sections: u16,
    characteristics: u16,
    opt_magic: u16,
    subsystem: u16,
}

/// Parse minimal PE headers from binary data to verify architecture integrity
fn parse_pe_header(data: &[u8]) -> Result<PeHeaderInfo, String> {
    if data.len() < 0x40 {
        return Err("File too small to contain DOS header".to_string());
    }

    // 1. Verify DOS signature 'MZ' (0x5A4D)
    if data[0] != b'M' || data[1] != b'Z' {
        return Err("Missing DOS 'MZ' signature".to_string());
    }

    // 2. Read e_lfanew offset to PE signature at offset 0x3C
    let e_lfanew = u32::from_le_bytes([data[0x3C], data[0x3D], data[0x3E], data[0x3F]]) as usize;
    if e_lfanew + 24 > data.len() {
        return Err(format!("e_lfanew offset {:#x} out of range", e_lfanew));
    }

    // 3. Verify PE signature 'PE\0\0' (0x00004550)
    if &data[e_lfanew..e_lfanew + 4] != b"PE\0\0" {
        return Err("Missing 'PE\\0\\0' signature".to_string());
    }

    // 4. Read COFF File Header (20 bytes starting at e_lfanew + 4)
    let coff_offset = e_lfanew + 4;
    let machine = u16::from_le_bytes([data[coff_offset], data[coff_offset + 1]]);
    let number_of_sections = u16::from_le_bytes([data[coff_offset + 2], data[coff_offset + 3]]);
    let size_of_opt_header = u16::from_le_bytes([data[coff_offset + 16], data[coff_offset + 17]]) as usize;
    let characteristics = u16::from_le_bytes([data[coff_offset + 18], data[coff_offset + 19]]);

    // 5. Read Optional Header (starting at e_lfanew + 24)
    let opt_offset = e_lfanew + 24;
    if opt_offset + size_of_opt_header > data.len() || size_of_opt_header < 68 {
        return Err("Optional header truncated or missing".to_string());
    }

    let opt_magic = u16::from_le_bytes([data[opt_offset], data[opt_offset + 1]]);
    // Subsystem is at offset 68 in Optional Header (for both PE32 and PE32+)
    let subsystem = u16::from_le_bytes([data[opt_offset + 68], data[opt_offset + 69]]);

    Ok(PeHeaderInfo {
        machine,
        number_of_sections,
        characteristics,
        opt_magic,
        subsystem,
    })
}

const IMAGE_FILE_MACHINE_AMD64: u16 = 0x8664;
const PE32PLUS_MAGIC: u16 = 0x020B;
const IMAGE_FILE_DLL: u16 = 0x2000;
const IMAGE_SUBSYSTEM_NATIVE: u16 = 1;
const IMAGE_SUBSYSTEM_WINDOWS_GUI: u16 = 2;
const IMAGE_SUBSYSTEM_WINDOWS_CUI: u16 = 3;

#[test]
fn test_driver_directory_structure_and_file_presence() {
    let driver_dir = get_driver_dir();
    assert!(driver_dir.exists(), "Driver staging directory '{:?}' must exist", driver_dir);

    let required_files = [
        "VirtualDisplayDriver.cer",
        "VirtualDisplayDriver.inf",
        "vdd.exe",
        "install_driver.bat",
        "uninstall_driver.bat",
        "x64/VirtualDisplayDriver.sys",
        "x64/VirtualDisplayDriver.dll",
    ];

    for relative_path in &required_files {
        let file_path = driver_dir.join(relative_path);
        assert!(
            file_path.exists(),
            "Required staged artifact '{:?}' must exist",
            file_path
        );

        let metadata = fs::metadata(&file_path)
            .unwrap_or_else(|_| panic!("Failed to read metadata for {:?}", file_path));
        assert!(
            metadata.len() > 0,
            "Staged artifact '{:?}' must not be empty (size was 0 bytes)",
            file_path
        );
    }
}

#[test]
fn test_x64_pe_architecture_integrity() {
    let driver_dir = get_driver_dir();

    // 1. Verify vdd.exe companion IPC service binary
    let vdd_path = driver_dir.join("vdd.exe");
    let vdd_bytes = fs::read(&vdd_path).expect("Failed to read vdd.exe");
    let vdd_pe = parse_pe_header(&vdd_bytes).expect("vdd.exe must have valid PE header");

    assert_eq!(
        vdd_pe.machine, IMAGE_FILE_MACHINE_AMD64,
        "vdd.exe must be compiled for x64 AMD64 (0x8664)"
    );
    assert_eq!(
        vdd_pe.opt_magic, PE32PLUS_MAGIC,
        "vdd.exe must use PE32+ (64-bit) optional header format"
    );
    assert!(
        vdd_pe.subsystem == IMAGE_SUBSYSTEM_WINDOWS_CUI || vdd_pe.subsystem == IMAGE_SUBSYSTEM_WINDOWS_GUI,
        "vdd.exe subsystem must be Windows CUI (3) or GUI (2), got {}",
        vdd_pe.subsystem
    );

    // 2. Verify x64/VirtualDisplayDriver.sys kernel-mode driver
    let sys_path = driver_dir.join("x64").join("VirtualDisplayDriver.sys");
    let sys_bytes = fs::read(&sys_path).expect("Failed to read VirtualDisplayDriver.sys");
    let sys_pe = parse_pe_header(&sys_bytes).expect("VirtualDisplayDriver.sys must have valid PE header");

    assert_eq!(
        sys_pe.machine, IMAGE_FILE_MACHINE_AMD64,
        "VirtualDisplayDriver.sys must be compiled for x64 AMD64 (0x8664)"
    );
    assert_eq!(
        sys_pe.opt_magic, PE32PLUS_MAGIC,
        "VirtualDisplayDriver.sys must use PE32+ (64-bit) optional header format"
    );
    assert_eq!(
        sys_pe.subsystem, IMAGE_SUBSYSTEM_NATIVE,
        "VirtualDisplayDriver.sys must have Native subsystem (1)"
    );

    // 3. Verify x64/VirtualDisplayDriver.dll user-mode driver
    let dll_path = driver_dir.join("x64").join("VirtualDisplayDriver.dll");
    let dll_bytes = fs::read(&dll_path).expect("Failed to read VirtualDisplayDriver.dll");
    let dll_pe = parse_pe_header(&dll_bytes).expect("VirtualDisplayDriver.dll must have valid PE header");

    assert_eq!(
        dll_pe.machine, IMAGE_FILE_MACHINE_AMD64,
        "VirtualDisplayDriver.dll must be compiled for x64 AMD64 (0x8664)"
    );
    assert_eq!(
        dll_pe.opt_magic, PE32PLUS_MAGIC,
        "VirtualDisplayDriver.dll must use PE32+ (64-bit) optional header format"
    );
    assert!(
        (dll_pe.characteristics & IMAGE_FILE_DLL) != 0,
        "VirtualDisplayDriver.dll characteristics must include IMAGE_FILE_DLL (0x2000)"
    );
}

#[test]
fn test_driver_inf_manifest_validity() {
    let driver_dir = get_driver_dir();
    let inf_path = driver_dir.join("VirtualDisplayDriver.inf");
    let inf_content = fs::read_to_string(&inf_path).expect("Failed to read VirtualDisplayDriver.inf");

    // Check required INF sections
    let required_sections = [
        "[Version]",
        "[DestinationDirs]",
        "[SourceDisksNames]",
        "[SourceDisksFiles]",
        "[Manufacturer]",
        "[Strings]",
    ];
    for section in &required_sections {
        assert!(
            inf_content.contains(section),
            "VirtualDisplayDriver.inf missing required section '{}'",
            section
        );
    }

    // Check Class and ClassGuid for Display
    assert!(
        inf_content.contains("Class=Display") || inf_content.contains("Class = Display"),
        "INF must declare Class=Display"
    );
    assert!(
        inf_content.to_lowercase().contains("{4d36e968-e325-11ce-bfc1-08002be10318}"),
        "INF must declare standard Display ClassGuid {{4d36e968-e325-11ce-bfc1-08002be10318}}"
    );

    // Check Hardware IDs
    assert!(
        inf_content.contains("Root\\VirtualDisplayDriver"),
        "INF must specify hardware ID Root\\VirtualDisplayDriver"
    );
    assert!(
        inf_content.contains("*VDD0001"),
        "INF must specify hardware ID *VDD0001"
    );

    // Check Driver binaries reference
    assert!(
        inf_content.contains("VirtualDisplayDriver.dll"),
        "INF must reference UMDF binary VirtualDisplayDriver.dll"
    );
    assert!(
        inf_content.contains("VirtualDisplayDriver.sys"),
        "INF must reference KMDF binary VirtualDisplayDriver.sys"
    );

    // Check UMDF service definition
    assert!(
        inf_content.contains("UmdfService=VirtualDisplayDriver")
            || inf_content.contains("UmdfService = VirtualDisplayDriver"),
        "INF must configure UmdfService=VirtualDisplayDriver"
    );
}

#[test]
fn test_staged_certificate_validity() {
    let driver_dir = get_driver_dir();
    let cer_path = driver_dir.join("VirtualDisplayDriver.cer");
    let cer_bytes = fs::read(&cer_path).expect("Failed to read VirtualDisplayDriver.cer");

    assert!(
        cer_bytes.len() >= 128,
        "VirtualDisplayDriver.cer must be a non-trivial certificate file (>= 128 bytes, was {})",
        cer_bytes.len()
    );

    // Check DER format (ASN.1 SEQUENCE starts with 0x30, 0x82) or PEM header
    let is_der = cer_bytes.starts_with(&[0x30, 0x82]) || cer_bytes.starts_with(&[0x30, 0x81]);
    let is_pem = cer_bytes.starts_with(b"-----BEGIN CERTIFICATE-----");
    assert!(
        is_der || is_pem,
        "VirtualDisplayDriver.cer must be valid DER or PEM encoded X.509 certificate"
    );
}

#[test]
fn test_install_driver_batch_script_workflow() {
    let driver_dir = get_driver_dir();
    let install_path = driver_dir.join("install_driver.bat");
    let script = fs::read_to_string(&install_path).expect("Failed to read install_driver.bat");

    // 1. UAC self-elevation check
    assert!(
        script.contains("net session"),
        "install_driver.bat must test for admin privileges via net session"
    );
    assert!(
        script.contains("Start-Process -FilePath '%~dpnx0' -Verb RunAs"),
        "install_driver.bat must request UAC elevation via Start-Process"
    );

    // 2. Certificate trust import
    assert!(
        script.contains("certutil -addstore \"TrustedPublisher\""),
        "install_driver.bat must import certificate into TrustedPublisher store"
    );
    assert!(
        script.contains("certutil -addstore \"Root\""),
        "install_driver.bat must import certificate into Root store"
    );

    // 3. Driver INF installation via pnputil
    assert!(
        script.contains("pnputil /add-driver") && script.contains("/install"),
        "install_driver.bat must register driver with 'pnputil /add-driver ... /install'"
    );

    // 4. Companion service registration
    assert!(
        script.contains("vdd.exe\" install") || script.contains("vdd.exe install"),
        "install_driver.bat must register companion service with 'vdd.exe install'"
    );

    // 5. Registry markers gated on successful installation
    assert!(
        script.contains("HKLM\\SOFTWARE\\VirtualDisplayDriver"),
        "install_driver.bat must write to HKLM\\SOFTWARE\\VirtualDisplayDriver"
    );
    assert!(
        script.contains("!DRIVER_INSTALLED! equ 1"),
        "install_driver.bat must gate registry marker on DRIVER_INSTALLED == 1"
    );
}

#[test]
fn test_uninstall_driver_batch_script_workflow() {
    let driver_dir = get_driver_dir();
    let uninstall_path = driver_dir.join("uninstall_driver.bat");
    let script = fs::read_to_string(&uninstall_path).expect("Failed to read uninstall_driver.bat");

    // 1. UAC self-elevation check
    assert!(
        script.contains("net session"),
        "uninstall_driver.bat must test for admin privileges via net session"
    );
    assert!(
        script.contains("Start-Process -FilePath '%~dpnx0' -Verb RunAs"),
        "uninstall_driver.bat must request UAC elevation via Start-Process"
    );

    // 2. Companion service teardown
    assert!(
        script.contains("vdd.exe\" uninstall") || script.contains("vdd.exe uninstall"),
        "uninstall_driver.bat must uninstall companion service with 'vdd.exe uninstall'"
    );

    // 3. Driver store package removal
    assert!(
        script.contains("pnputil /delete-driver"),
        "uninstall_driver.bat must remove drivers from store via pnputil /delete-driver"
    );
    assert!(
        script.contains("/uninstall /force"),
        "uninstall_driver.bat must use /uninstall /force with pnputil /delete-driver"
    );

    // 4. Certificate cleanup
    assert!(
        script.contains("certutil -delstore"),
        "uninstall_driver.bat must clean up installed certificates via certutil -delstore"
    );

    // 5. Registry cleanup
    assert!(
        script.contains("reg delete \"HKLM\\SOFTWARE\\VirtualDisplayDriver\" /f"),
        "uninstall_driver.bat must delete registry keys on uninstall"
    );
}
