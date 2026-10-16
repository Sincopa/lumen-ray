@echo off
setlocal EnableExtensions
chcp 65001 >nul
title Lumen Ray - Installer Build
cd /d "%~dp0"

echo ============================================================
echo    Lumen Ray - Installer Build (.exe Setup)
echo ============================================================
echo.

where node >nul 2>&1
if errorlevel 1 (
    echo [ERROR] Node.js not found in PATH.
    echo Please install Node.js from https://nodejs.org/
    pause
    exit /b 1
)

where cargo >nul 2>&1
if errorlevel 1 (
    echo [ERROR] Rust/Cargo not found in PATH.
    echo Please install Rust from https://rustup.rs/
    pause
    exit /b 1
)

if not exist "node_modules\" (
    echo [INFO] Installing npm dependencies...
    call npm install
    if errorlevel 1 (
        echo [ERROR] npm install failed.
        pause
        exit /b 1
    )
)

powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0src-tauri\build\build-release.ps1" -Mode Installer
if errorlevel 1 (
    echo.
    echo ============================================================
    echo    [FAILED] Installer build failed. Check errors above.
    echo ============================================================
    echo.
    pause
    exit /b 1
)

if exist "%~dp0release\" (
    start "" explorer.exe "%~dp0release"
)

echo.
echo Press any key to exit...
pause >nul
exit /b 0
