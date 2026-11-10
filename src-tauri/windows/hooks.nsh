!macro NSIS_HOOK_PREUNINSTALL
  InitPluginsDir
  CopyFiles /SILENT "$INSTDIR\${MAINBINARYNAME}.exe" "$PLUGINSDIR\zastava-cleanup.exe"
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ${If} ${FileExists} "$PLUGINSDIR\zastava-cleanup.exe"
    ExecWait '"$PLUGINSDIR\zastava-cleanup.exe" --cleanup'
    Delete "$PLUGINSDIR\zastava-cleanup.exe"
  ${EndIf}
!macroend
