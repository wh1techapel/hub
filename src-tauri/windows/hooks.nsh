; Hub's installer hooks (tauri.conf.json, bundle.windows.nsis.installerHooks).

; Claude Code keeps hub-mcp.exe running while a session has it open, and a running file cannot be
; replaced: the 1.0.0 update left the old bridge in place without a word (F2, 2026-11-09). Stop
; every copy first; Claude Code starts the bridge again when it next needs Hub.
!macro HUB_STOP_BRIDGE
  nsExec::Exec 'taskkill /F /IM hub-mcp.exe'
  Pop $0
  Sleep 500
!macroend

; A per-user install goes to %LOCALAPPDATA%\Hub by default, which is Hub's data folder (the
; owner's choice, CONTEXT.md CP2). The program goes beside it instead, in
; %LOCALAPPDATA%\Programs\Hub (where per-user programs live), unless the owner chose another
; folder. Hub's data is never inside the program's folder, so updating or removing Hub never
; touches it.
!macro NSIS_HOOK_PREINSTALL
  ${If} $INSTDIR == "$LOCALAPPDATA\${PRODUCTNAME}"
    StrCpy $INSTDIR "$LOCALAPPDATA\Programs\${PRODUCTNAME}"
    SetOutPath $INSTDIR
  ${EndIf}
  !insertmacro HUB_STOP_BRIDGE
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  !insertmacro HUB_STOP_BRIDGE
!macroend

; Studio's bridge for Claude Code (docs/studio.md, "MCP"): hub-mcp.exe goes beside hub.exe, where
; Hub finds it for Pasang ke Claude Code and where Claude Code starts it. Built by
; beforeBuildCommand (cargo build --release -p hub-mcp) before the installer is made.
!define HUB_MCP_EXE "${__FILEDIR__}\..\..\target\release\hub-mcp.exe"

!macro NSIS_HOOK_POSTINSTALL
  SetOutPath $INSTDIR
  File "${HUB_MCP_EXE}"
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  Delete "$INSTDIR\hub-mcp.exe"
!macroend
