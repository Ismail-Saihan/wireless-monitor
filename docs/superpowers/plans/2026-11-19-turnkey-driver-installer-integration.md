# Turnkey Driver Installer Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Integrate automated Windows Virtual Display Driver (IddCx) installation and uninstallation directly into the NSIS setup wizard and uninstaller.

**Architecture:** Create NSIS custom hook macros in `src-tauri/nsis/installerHooks.nsh` that invoke `install_driver.bat` during `NSIS_HOOK_POSTINSTALL` and `uninstall_driver.bat` during `NSIS_HOOK_PREUNINSTALL` using `nsExec::ExecToLog`; configure `tauri.conf.json` with `installMode: "perMachine"` and `installerHooks`; update verification suites, build scripts, and documentation; re-bundle `Wireless-Monitor-Setup.exe`.

**Tech Stack:** NSIS, Tauri 2.0, Rust, PowerShell, Python test harnesses.

**Spec:** `docs/superpowers/specs/2026-11-19-turnkey-driver-installer-integration.md`

## Global Constraints

- Installer must request administrative privileges once at startup (`installMode: "perMachine"`).
- Driver installation must execute silently without flashing popup console windows (`nsExec::ExecToLog`).
- Must probe candidate resource paths (`_up_/drivers/windows`, `resources/_up_/drivers/windows`, `drivers/windows`).
- Driver installation failure must warn and complete without crashing or rolling back the application.
- Uninstaller must cleanly remove driver, companion service, certificates, and registry entries.
- Zero regressions across existing unit, integration, and verification test suites.

## Review Focus

- Invalid NSIS macro syntax breaking `makensis` compilation during Tauri bundling.
- Resource path mismatch causing `install_driver.bat` not to be found after installation into `C:\Program Files\Wireless Monitor\`.
- Failure of uninstaller to clean up published OEM INF files or root certificates.
- Breakage of automated verification test suites (`verify_installers.py`, `verify_phase3.py`).
- Flashing command prompt windows during setup disrupting the seamless user experience.

---

### Task 1: Create NSIS Custom Hooks for Automated Driver Lifecycle

**Files:**
- Create: `src-tauri/nsis/installerHooks.nsh`
- Test: `src-tauri/tests/installer_packaging_test.rs`

**Interfaces:**
- Produces: NSIS macros `NSIS_HOOK_POSTINSTALL` and `NSIS_HOOK_PREUNINSTALL` executing driver setup and teardown.

- [ ] **Step 1: Write `src-tauri/nsis/installerHooks.nsh`**
Implement `NSIS_HOOK_POSTINSTALL` to detect the candidate driver paths, execute `install_driver.bat` with `nsExec::ExecToLog`, and log progress. Implement `NSIS_HOOK_PREUNINSTALL` to execute `uninstall_driver.bat` with `nsExec::ExecToLog`.

- [ ] **Step 2: Add unit test in `src-tauri/tests/installer_packaging_test.rs`**
Add test asserting that `installerHooks.nsh` exists, contains `NSIS_HOOK_POSTINSTALL`, `NSIS_HOOK_PREUNINSTALL`, `nsExec::ExecToLog`, and references `install_driver.bat` and `uninstall_driver.bat`.

- [ ] **Step 3: Run unit test**
Run: `cargo test --manifest-path src-tauri/Cargo.toml --test installer_packaging_test`
Expected: PASS

- [ ] **Step 4: Commit Task 1**
```bash
git add src-tauri/nsis/installerHooks.nsh src-tauri/tests/installer_packaging_test.rs
git commit -m "feat(installer): create NSIS hooks for automatic driver installation and clean teardown"
```

---

### Task 2: Configure Tauri Bundle for Per-Machine Install & Hooks

**Files:**
- Modify: `src-tauri/tauri.conf.json`

**Interfaces:**
- Consumes: `src-tauri/nsis/installerHooks.nsh`
- Produces: Updated Tauri NSIS bundle config with `installMode: "perMachine"` and `installerHooks: "nsis/installerHooks.nsh"`.

- [ ] **Step 1: Update `src-tauri/tauri.conf.json`**
Set `bundle.windows.nsis.installMode = "perMachine"` and add `"installerHooks": "nsis/installerHooks.nsh"`.

- [ ] **Step 2: Update verification script `src-tauri/tests/verify_installers.py`**
Update `test_tauri_bundle_configuration` in `verify_installers.py` to assert `installerHooks` is configured and `installMode` is `"perMachine"`.

- [ ] **Step 3: Run `verify_installers.py`**
Run: `python src-tauri/tests/verify_installers.py`
Expected: PASS (0 errors)

- [ ] **Step 4: Commit Task 2**
```bash
git add src-tauri/tauri.conf.json src-tauri/tests/verify_installers.py
git commit -m "feat(packaging): configure per-machine installation and installer hooks in tauri.conf.json"
```

---

### Task 3: Build & Package Turnkey Windows Installer

**Files:**
- Modify: `scripts/build-windows.ps1`
- Produces: `F:\Wireless Monitor Project\Wireless-Monitor-Setup.exe`
- Produces: `F:\Wireless Monitor Project\wireless-monitor.exe`

**Interfaces:**
- Consumes: Tauri CLI bundler with NSIS

- [ ] **Step 1: Run local NSIS build**
Run: `powershell -ExecutionPolicy Bypass -File .\scripts\build-windows.ps1 -Target nsis -SkipFrontend`
Expected: Successful `makensis` compilation and generation of `Wireless Monitor_0.1.0_x64-setup.exe`.

- [ ] **Step 2: Copy fresh executables to project root**
Copy `src-tauri/target/release/wireless-monitor.exe` and `Wireless Monitor_0.1.0_x64-setup.exe` to project root as `wireless-monitor.exe` and `Wireless-Monitor-Setup.exe`.

- [ ] **Step 3: Verify binary attributes**
Verify file sizes, timestamps, and NSIS hook integration.

- [ ] **Step 4: Commit Task 3**
```bash
git add scripts/build-windows.ps1
git commit -m "chore(installer): rebuild turnkey Windows setup installer with automated driver setup"
```

---

### Task 4: Synchronize Documentation and Release Workflow

**Files:**
- Modify: `INSTALL.md`
- Modify: `README.md`
- Modify: `AGENT.md`

**Interfaces:**
- Produces: Updated user guide clarifying that the setup installer now installs the driver automatically with zero post-install steps.

- [ ] **Step 1: Update `INSTALL.md`**
Update Windows installation section to emphasize that the setup wizard automatically installs the virtual display driver.

- [ ] **Step 2: Update `README.md` and `AGENT.md`**
Highlight turnkey driver setup in features list and agent architectural documentation.

- [ ] **Step 3: Run full verification suite**
Run: `python src-tauri/tests/verify_installers.py && python src-tauri/tests/verify_phase3.py`
Expected: ALL PASS (0 errors).

- [ ] **Step 4: Commit & Push to GitHub**
```bash
git add INSTALL.md README.md AGENT.md
git commit -m "docs: document turnkey automated driver installation in setup wizard"
git push origin main
```
