; Lumen Tunnel service (src/platform/service.rs).
; Lumen installs it on its first TUN connection; it runs a copy of Lumen from
; Program Files. Uninstalling Lumen removes it. An update keeps it: the next
; connection refreshes the copy.
!macro NSIS_HOOK_PREUNINSTALL
  ${If} $UpdateMode <> 1
    Push $R9
    ClearErrors
    ReadRegStr $R9 HKLM "SYSTEM\CurrentControlSet\Services\LumenTunnel" "ImagePath"
    ${IfNot} ${Errors}
      ExecShellWait "runas" "$INSTDIR\${MAINBINARYNAME}.exe" "--lumen-uninstall-service" SW_HIDE
    ${EndIf}
    Pop $R9
  ${EndIf}
!macroend
