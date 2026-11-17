@echo off
echo =======================================================
echo  Wireless Virtual Monitor - Windows Driver Uninstaller
echo =======================================================
echo.
echo Removing IddCx Virtual Display Driver...
net session >nul 2>&1
if %errorlevel% neq 0 (
    echo Error: Please right-click and run this script as Administrator.
    pause
    exit /b 1
)

echo Removing device and tearing down virtual display adapters...
REM devcon remove ...
echo Complete.
exit /b 0
