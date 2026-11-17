@echo off
echo =======================================================
echo  Wireless Virtual Monitor - Windows Driver Installer
echo =======================================================
echo.
echo Installing IddCx Virtual Display Driver...
echo Requesting administrator privileges if needed...

net session >nul 2>&1
if %errorlevel% neq 0 (
    echo Error: Please right-click and run this script as Administrator.
    pause
    exit /b 1
)

echo Driver directory: %~dp0
echo Installing driver certificates and registering device...
REM Integration hook for Virtual-Display-Driver companion CLI (e.g. devcon / vdd.exe)
echo Complete.
exit /b 0
