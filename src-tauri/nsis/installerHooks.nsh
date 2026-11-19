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

  DetailPrint "Configuring Windows Firewall rules for Wireless Monitor..."
  nsExec::ExecToLog 'netsh advfirewall firewall add rule name="Wireless Monitor" dir=in action=allow program="$INSTDIR\wireless-monitor.exe" enable=yes'
  Pop $0
  ${If} $0 != 0
    DetailPrint "Warning: Failed to add Windows Firewall rule (status $0)."
  ${Else}
    DetailPrint "Windows Firewall rule created successfully."
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

  DetailPrint "Removing Windows Firewall rules for Wireless Monitor..."
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="Wireless Monitor"'
  Pop $0
!macroend
