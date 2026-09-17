@echo off
setlocal EnableExtensions
chcp 65001 >nul
title Lumen - Installer Build
cd /d "%~dp0"

echo ============================================================
echo    Lumen - Installer Build (.exe setup + .msi)
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

echo [1/2] Building installer packages (NSIS setup + MSI)...
call npx tauri build
if errorlevel 1 goto :error

echo.
echo [2/2] Copying installers to output folder...
if not exist "installers\" mkdir "installers"

if exist "src-tauri\target\release\bundle\nsis\*.exe" (
    copy /Y "src-tauri\target\release\bundle\nsis\*.exe" "installers\" >nul
)

if exist "src-tauri\target\release\bundle\msi\*.msi" (
    copy /Y "src-tauri\target\release\bundle\msi\*.msi" "installers\" >nul
)

echo.
echo ============================================================
echo    [SUCCESS] Installers built successfully!
echo ============================================================
echo.
echo Output directory:
echo    %~dp0installers
echo.
echo Generated files:
dir /b "installers\"
echo.

start "" explorer.exe "%~dp0installers"

echo Press any key to exit...
pause >nul
exit /b 0

:error
echo.
echo ============================================================
echo    [FAILED] Installer build failed. Check errors above.
echo ============================================================
echo.
pause
exit /b 1
