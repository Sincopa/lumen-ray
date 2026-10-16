@echo off
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0src-tauri\build\diagnose-connection.ps1"
echo.
echo Diagnostic script finished. Press any key to close this window.
pause >nul
