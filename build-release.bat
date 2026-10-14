@echo off
setlocal EnableExtensions
chcp 65001 >nul
title Lumen - Release Build
cd /d "%~dp0"

echo ============================================================
echo    Lumen - Release Build (Standalone EXE)
echo ============================================================
echo.

where node >nul 2>&1
if errorlevel 1 (
    echo [ERROR] Node.js not found in PATH.
    echo Please install Node.js from https://nodejs.org/
    goto :error
)

where cargo >nul 2>&1
if errorlevel 1 (
    echo [ERROR] Rust/Cargo not found in PATH.
    echo Please install Rust from https://rustup.rs/
    goto :error
)

if not exist "node_modules\" (
    echo [INFO] Installing npm dependencies...
    call npm install
    if errorlevel 1 goto :error
)

taskkill /F /IM app.exe >nul 2>&1
taskkill /F /IM lumen.exe >nul 2>&1
taskkill /F /IM misc-vpn.exe >nul 2>&1

echo [1/2] Building release application...
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\build-release.ps1" -Mode Portable
if errorlevel 1 goto :error
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\sign-windows.ps1" -Mode Sign -Path "%~dp0src-tauri\target\release\app.exe"
if errorlevel 1 goto :error

echo.
echo [2/2] Copying executable to release folder...
if not exist "release\" mkdir "release"
copy /Y "src-tauri\target\release\app.exe" "release\lumen.exe" >nul
if errorlevel 1 (
    echo [ERROR] Failed to copy binary to release\lumen.exe
    goto :error
)
if not exist "release\runtime\" mkdir "release\runtime"
xcopy /E /I /Y "src-tauri\runtime\*" "release\runtime\" >nul
if errorlevel 1 goto :error

echo.
echo ============================================================
echo    [SUCCESS] Release version built successfully!
echo ============================================================
echo.
echo Output file:
echo    %~dp0release\lumen.exe
echo.

start "" explorer.exe "%~dp0release"

echo Press any key to exit...
pause >nul
exit /b 0

:error
echo.
echo ============================================================
echo    [FAILED] Build failed. Check errors above.
echo ============================================================
echo.
pause
exit /b 1
