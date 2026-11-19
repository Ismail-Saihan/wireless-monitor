# Turnkey Driver Installer Integration Design Spec

## 1. Overview & Problem Statement

Currently, when a user installs **Wireless Virtual Monitor** via the Windows NSIS Setup Wizard (`Wireless-Monitor-Setup.exe`), the installer only extracts the application binary and creates desktop/Start Menu shortcuts. The required Windows Virtual Display Driver (IddCx) and companion services were not installed during the setup process. Users were required to manually click an in-app button or execute external batch scripts after installation.

This specification details the architecture for an end-to-end turnkey Windows installer that **automatically and silently installs the virtual display driver and certificates during the setup wizard execution** and **cleanly tears them down upon uninstallation**.

---

## 2. Core Decisions & Architectural Requirements

1. **Privilege & Installation Model**:
   - The NSIS installer enforces **Per-Machine Installation (`installMode: "perMachine"`)** requiring a single User Account Control (UAC) prompt at installer startup (`RequestExecutionLevel admin`).
   - Installs to `C:\Program Files\Wireless Monitor\` so the driver and app are universally available to all system accounts.

2. **Silent & Seamless In-Setup Driver Installation (`NSIS_HOOK_POSTINSTALL`)**:
   - Driver installation occurs automatically during the NSIS file extraction and registration phase.
   - Uses `nsExec::ExecToLog` to execute the driver installation batch script silently in the background, piping output to the installer detail log with **zero flashing console windows**.
   - Displays clear progress status: `"Installing Virtual Display Driver and Certificates..."`.

3. **Driver Staging Path Discovery**:
   - Probes the candidate resource unpack paths created by Tauri 2.0:
     - `$INSTDIR\_up_\drivers\windows\install_driver.bat`
     - `$INSTDIR\resources\_up_\drivers\windows\install_driver.bat`
     - `$INSTDIR\drivers\windows\install_driver.bat`
     - `$INSTDIR\resources\drivers\windows\install_driver.bat`

4. **Resilient Error Handling ("Warn & Complete")**:
   - If driver installation fails (e.g. strict enterprise Group Policy or antivirus hook), the installer logs a warning, does not rollback or crash the application installation, and allows the user to run Receiver mode or retry driver setup within the app.

5. **Clean Uninstallation Teardown (`NSIS_HOOK_PREUNINSTALL`)**:
   - When the app is uninstalled via Windows Installed Apps / Control Panel, the uninstaller automatically runs `uninstall_driver.bat` via `nsExec::ExecToLog`.
   - Stops and unregisters `vdd.exe` companion service.
   - Deletes published driver packages (`pnputil /delete-driver ... /uninstall /force`).
   - Removes signing certificates from `TrustedPublisher` and `Root` stores.
   - Cleans up registry entries under `HKLM\SOFTWARE\VirtualDisplayDriver`.

6. **Application UI Synchronization**:
   - Because the setup wizard completes driver installation, the UI dashboard detects `driver_installed == true` immediately upon first launch, displaying a green **Driver Active** badge.
   - The manual install button remains available as a graceful fallback for portable execution.

---

## 3. Component Details & Interfaces

### 3.1 NSIS Installer Hooks (`src-tauri/nsis/installerHooks.nsh`)

```nsis
!macro NSIS_HOOK_POSTINSTALL
  DetailPrint "Installing Virtual Display Driver..."
  Var /GLOBAL DRIVER_INSTALL_SCRIPT
  StrCpy $DRIVER_INSTALL_SCRIPT ""

  ${If} ${FileExists} "$INSTDIR\_up_\drivers\windows\install_driver.bat"
    StrCpy $DRIVER_INSTALL_SCRIPT "$INSTDIR\_up_\drivers\windows\install_driver.bat"
  ${ElseIf} ${FileExists} "$INSTDIR\resources\_up_\drivers\windows\install_driver.bat"
    StrCpy $DRIVER_INSTALL_SCRIPT "$INSTDIR\resources\_up_\drivers\windows\install_driver.bat"
  ${ElseIf} ${FileExists} "$INSTDIR\drivers\windows\install_driver.bat"
    StrCpy $DRIVER_INSTALL_SCRIPT "$INSTDIR\drivers\windows\install_driver.bat"
  ${ElseIf} ${FileExists} "$INSTDIR\resources\drivers\windows\install_driver.bat"
    StrCpy $DRIVER_INSTALL_SCRIPT "$INSTDIR\resources\drivers\windows\install_driver.bat"
  ${EndIf}

  ${If} $DRIVER_INSTALL_SCRIPT != ""
    nsExec::ExecToLog '"$DRIVER_INSTALL_SCRIPT"'
    Pop $0
    ${If} $0 != 0
      DetailPrint "Warning: Driver setup returned status $0. You can still use Receiver mode or retry in app."
    ${Else}
      DetailPrint "Virtual Display Driver installed and registered successfully."
    ${EndIf}
  ${Else}
    DetailPrint "Note: Standalone driver installer script not found in installation directory."
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  DetailPrint "Uninstalling Virtual Display Driver..."
  Var /GLOBAL DRIVER_UNINSTALL_SCRIPT
  StrCpy $DRIVER_UNINSTALL_SCRIPT ""

  ${If} ${FileExists} "$INSTDIR\_up_\drivers\windows\uninstall_driver.bat"
    StrCpy $DRIVER_UNINSTALL_SCRIPT "$INSTDIR\_up_\drivers\windows\uninstall_driver.bat"
  ${ElseIf} ${FileExists} "$INSTDIR\resources\_up_\drivers\windows\uninstall_driver.bat"
    StrCpy $DRIVER_UNINSTALL_SCRIPT "$INSTDIR\resources\_up_\drivers\windows\uninstall_driver.bat"
  ${ElseIf} ${FileExists} "$INSTDIR\drivers\windows\uninstall_driver.bat"
    StrCpy $DRIVER_UNINSTALL_SCRIPT "$INSTDIR\drivers\windows\uninstall_driver.bat"
  ${ElseIf} ${FileExists} "$INSTDIR\resources\drivers\windows\uninstall_driver.bat"
    StrCpy $DRIVER_UNINSTALL_SCRIPT "$INSTDIR\resources\drivers\windows\uninstall_driver.bat"
  ${EndIf}

  ${If} $DRIVER_UNINSTALL_SCRIPT != ""
    nsExec::ExecToLog '"$DRIVER_UNINSTALL_SCRIPT"'
    Pop $0
  ${EndIf}
!macroend
```

### 3.2 Tauri Configuration (`src-tauri/tauri.conf.json`)

```json
"windows": {
  "nsis": {
    "installMode": "perMachine",
    "languages": [
      "English"
    ],
    "displayLanguageSelector": false,
    "installerHooks": "nsis/installerHooks.nsh"
  }
}
```

---

## 4. Verification Plan

1. **Static Validation**:
   - `python src-tauri/tests/verify_installers.py` updated to check for `installerHooks.nsh`, `installerHooks` path in `tauri.conf.json`, and `installMode: "perMachine"`.
2. **Build Validation**:
   - Re-build the production Windows NSIS installer using `.\scripts\build-windows.ps1 -Target nsis`.
   - Ensure NSIS packages without warnings or syntax errors.
3. **Execution Validation**:
   - Inspect installer bytecode to confirm `NSIS_HOOK_POSTINSTALL` and `NSIS_HOOK_PREUNINSTALL` are compiled into the binary.
   - Verify registry markers and pnputil execution.
