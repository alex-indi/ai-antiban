!macro NSIS_HOOK_PREUNINSTALL
  InitPluginsDir
  CopyFiles /SILENT "$INSTDIR\${MAINBINARYNAME}.exe" "$PLUGINSDIR\zastava-cleanup.exe"
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ${If} ${FileExists} "$PLUGINSDIR\zastava-cleanup.exe"
    ExecWait '"$PLUGINSDIR\zastava-cleanup.exe" --cleanup'
    Delete "$PLUGINSDIR\zastava-cleanup.exe"
  ${EndIf}
  ; Directly purge any legacy TZ, locale, or proxy variables from HKCU\Environment and WinINet
  DeleteRegValue HKCU "Environment" "TZ"
  DeleteRegValue HKCU "Environment" "LANG"
  DeleteRegValue HKCU "Environment" "LC_TIME"
  DeleteRegValue HKCU "Environment" "LC_ALL"
  DeleteRegValue HKCU "Environment" "HTTP_PROXY"
  DeleteRegValue HKCU "Environment" "HTTPS_PROXY"
  DeleteRegValue HKCU "Environment" "ALL_PROXY"
  DeleteRegValue HKCU "Environment" "NO_PROXY"
  DeleteRegValue HKCU "Environment" "http_proxy"
  DeleteRegValue HKCU "Environment" "https_proxy"
  DeleteRegValue HKCU "Environment" "all_proxy"
  DeleteRegValue HKCU "Environment" "no_proxy"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Internet Settings" "AutoConfigURL"
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Internet Settings" "ProxyEnable" 0
  ; Broadcast WM_SETTINGCHANGE ("Environment") so explorer.exe reloads environment immediately without reboot
  SendMessage ${HWND_BROADCAST} ${WM_WININICHANGE} 0 "STR:Environment" /TIMEOUT=2000
!macroend
