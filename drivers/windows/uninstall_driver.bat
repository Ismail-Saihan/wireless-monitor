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
    echo Requesting User Account Control (UAC) elevation...
    powershell -NoProfile -Command "Start-Process cmd.exe -ArgumentList '/c \"\"%~dpnx0\"\"' -Verb RunAs"
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

:: 3. Remove staged driver packages using pnputil
echo [2/3] Removing driver INF packages...
for %%i in ("%~dp0*.inf") do (
    echo Deleting driver package: %%~nxi
    pnputil /delete-driver "%%~nxi" /uninstall /force >nul 2>&1
)

:: 4. Clean up registry keys
echo [3/3] Removing registry entries (HKLM\SOFTWARE\VirtualDisplayDriver)...
reg delete "HKLM\SOFTWARE\VirtualDisplayDriver" /f >nul 2>&1

echo.
echo =======================================================
echo  Virtual Display Driver Cleanly Uninstalled!
echo =======================================================
exit /b 0
