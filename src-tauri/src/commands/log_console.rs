use crate::domain::errors::{BackendError, Result};
use tauri::Manager;

#[tauri::command]
pub fn open_log_console(app: tauri::AppHandle) -> Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        use windows_sys::Win32::System::Threading::CREATE_NEW_CONSOLE;

        let log_path = app
            .path()
            .app_log_dir()
            .map_err(|_| BackendError::new("LOG_CONSOLE_FAILED", "Log directory unavailable"))?
            .join(format!("{}.log", app.package_info().name));
        log::info!("Opening live log console");
        let script = concat!(
            "$Host.UI.RawUI.WindowTitle = 'Lumen logs'; ",
            "Write-Host 'Lumen - live application logs' -ForegroundColor Cyan; ",
            "Write-Host $env:LUMEN_LOG_PATH -ForegroundColor DarkGray; ",
            "Write-Host 'Close this window when testing is finished.' -ForegroundColor DarkGray; ",
            "while (-not (Test-Path -LiteralPath $env:LUMEN_LOG_PATH)) { Start-Sleep -Milliseconds 100 }; ",
            "Get-Content -LiteralPath $env:LUMEN_LOG_PATH -Tail 200 -Wait"
        );
        std::process::Command::new("powershell.exe")
            .args(["-NoLogo", "-NoProfile", "-Command", script])
            .env("LUMEN_LOG_PATH", log_path)
            .creation_flags(CREATE_NEW_CONSOLE)
            .spawn()
            .map(|_| ())
            .map_err(|_| BackendError::new("LOG_CONSOLE_FAILED", "Could not open log console"))
    }
    #[cfg(not(windows))]
    {
        let _ = app;
        Err(BackendError::new(
            "LOG_CONSOLE_FAILED",
            "Log console is only available on Windows",
        ))
    }
}
