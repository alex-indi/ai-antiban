!macro NSIS_HOOK_PREUNINSTALL
  InitPluginsDir
  CopyFiles /SILENT "$INSTDIR\${MAINBINARYNAME}.exe" "$PLUGINSDIR\zastava-cleanup.exe"
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; Только бинарник знает журнал владения. Без него неизвестные настройки сохраняются.
  ${If} ${FileExists} "$PLUGINSDIR\zastava-cleanup.exe"
    ClearErrors
    ExecWait '"$PLUGINSDIR\zastava-cleanup.exe" --cleanup' $0
    ${If} ${Errors}
      DetailPrint "Zastava: безопасное восстановление не запущено; настройки сохранены."
    ${ElseIf} $0 != 0
      DetailPrint "Zastava: восстановление не завершено; локальный журнал сохранён."
    ${EndIf}
    Delete "$PLUGINSDIR\zastava-cleanup.exe"
  ${EndIf}
!macroend
