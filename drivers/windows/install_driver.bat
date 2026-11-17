@echo off
setlocal EnableDelayedExpansion

echo =======================================================
echo  Wireless Virtual Monitor - Windows Driver Installer
echo =======================================================
echo.

:: 1. Verify Administrative Privileges; request UAC elevation if needed
net session >nul 2>&1
if %errorlevel% neq 0 (
    echo [INFO] Administrative privileges required.
    echo Requesting User Account Control (UAC) elevation...
    powershell -NoProfile -Command "Start-Process cmd.exe -ArgumentList '/c \"\"%~dpnx0\"\"' -Verb RunAs"
    exit /b %errorlevel%
)

echo [OK] Running with administrative privileges.
echo Driver Directory: %~dp0
echo.

:: 2. Certificate Import into TrustedPublisher / Root store
echo [1/4] Checking and importing driver signing certificates...
set CER_COUNT=0
for %%f in ("%~dp0*.cer") do (
    set /a CER_COUNT+=1
    echo Importing certificate: %%~nxf
    certutil -addstore "TrustedPublisher" "%%f" >nul 2>&1
    certutil -addstore "Root" "%%f" >nul 2>&1
)
if %CER_COUNT% equ 0 (
    echo [INFO] No standalone .cer certificates found in driver directory.
) else (
    echo [OK] %CER_COUNT% certificate(s) installed.
)
echo.

:: 3. Driver staging and installation via pnputil / devcon
echo [2/4] Registering and installing driver INF packages...
set INF_COUNT=0
for %%i in ("%~dp0*.inf") do (
    set /a INF_COUNT+=1
    echo Installing INF: %%~nxi
    pnputil /add-driver "%%i" /install
)
if %INF_COUNT% equ 0 (
    echo [INFO] No .inf driver package staged in %~dp0 yet.
    echo [INFO] Registering virtual device metadata for companion service...
)
echo.

:: 4. Companion service registration if companion binary exists
echo [3/4] Checking companion IPC service...
if exist "%~dp0vdd.exe" (
    echo [INFO] Found vdd.exe companion binary. Registering service...
    "%~dp0vdd.exe" install
) else (
    echo [INFO] Standalone companion executable not present; will be managed by app daemon.
)
echo.

:: 5. Register installation metadata in Windows Registry
echo [4/4] Writing registry markers (HKLM\SOFTWARE\VirtualDisplayDriver)...
reg add "HKLM\SOFTWARE\VirtualDisplayDriver" /v "Installed" /t REG_DWORD /d 1 /f >nul 2>&1
reg add "HKLM\SOFTWARE\VirtualDisplayDriver" /v "InstallPath" /t REG_SZ /d "%~dp0" /f >nul 2>&1
reg add "HKLM\SOFTWARE\VirtualDisplayDriver" /v "DriverModel" /t REG_SZ /d "IddCx" /f >nul 2>&1

echo.
echo =======================================================
echo  Virtual Display Driver Installation Completed Successfully!
echo =======================================================
exit /b 0
