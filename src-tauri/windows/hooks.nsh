; Installer hooks for Unhoard: the uninstaller removes the daily measurement's scheduled
; task of the current user, which Unhoard created when background insights were on.
!macro NSIS_HOOK_PREUNINSTALL
  nsExec::Exec '"$SYSDIR\schtasks.exe" /Delete /TN "Unhoard daily measurement" /F'
!macroend
