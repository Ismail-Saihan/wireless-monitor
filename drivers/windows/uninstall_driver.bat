@echo off
setlocal EnableDelayedExpansion

echo =======================================================
echo  Wireless Virtual Monitor - Windows Driver Uninstaller
echo =======================================================
echo.

:: 1. Verify Administrative Privileges; request UAC elevation if needed
net session >nul 2>&1
if %errorlevel% neq 0 (
    echo [INFO] Administrative privileges required.
    echo Requesting User Account Control [UAC] elevation...
    powershell -NoProfile -Command "$ws = Start-Process -FilePath '%~dpnx0' -Verb RunAs -Wait -PassThru; exit $ws.ExitCode"
    exit /b %errorlevel%
)

echo [OK] Running with administrative privileges.
echo.

:: 2. Stop and remove companion service if present
echo [1/3] Tearing down companion services and active virtual adapters...
if exist "%~dp0vdd.exe" (
    echo Stopping and uninstalling vdd.exe companion service...
    "%~dp0vdd.exe" uninstall
)

:: 3. Remove staged driver packages from Driver Store using published OEM names
echo [2/3] Removing driver packages from Driver Store...
for %%i in ("%~dp0*.inf") do (
    set "INF_ORIG=%%~nxi"
    echo Querying published OEM name for !INF_ORIG!...
    for /f "usebackq delims=" %%o in (`powershell -NoProfile -Command "$name = '%%~nxi'; (pnputil /enum-drivers | Select-String -Pattern 'Published Name:\s+(oem\d+\.inf)' -Context 0,2 | Where-Object { $_.Context.PostContext -match $name } | ForEach-Object { $_.Matches[0].Groups[1].Value })"`) do (
        if not "%%o"=="" (
            echo Deleting published driver package %%o [original: !INF_ORIG!]...
            pnputil /delete-driver "%%o" /uninstall /force >nul 2>&1
        )
    )
)

:: Also clean up any lingering VirtualDisplayDriver / IddSampleDriver packages
for /f "usebackq delims=" %%o in (`powershell -NoProfile -Command "(pnputil /enum-drivers | Select-String -Pattern 'Published Name:\s+(oem\d+\.inf)' -Context 0,3 | Where-Object { $_.Context.PostContext -match 'VirtualDisplay|IddSampleDriver' } | ForEach-Object { $_.Matches[0].Groups[1].Value })"`) do (
    if not "%%o"=="" (
        echo Deleting published driver package %%o...
        pnputil /delete-driver "%%o" /uninstall /force >nul 2>&1
    )
)

:: 4. Clean up registry keys
echo [3/3] Removing registry entries [HKLM\SOFTWARE\VirtualDisplayDriver]...
reg delete "HKLM\SOFTWARE\VirtualDisplayDriver" /f >nul 2>&1

echo.
echo =======================================================
echo  Virtual Display Driver Cleanly Uninstalled!
echo =======================================================
exit /b 0
