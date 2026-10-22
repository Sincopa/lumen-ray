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
    #[cfg(target_os = "macos")]
    {
        let log_path = app
            .path()
            .app_log_dir()
            .map_err(|_| BackendError::new("LOG_CONSOLE_FAILED", "Log directory unavailable"))?
            .join(format!("{}.log", app.package_info().name));
        log::info!("Opening live log console");
        // Console follows the file live; before the first write, show the folder instead.
        let mut command = std::process::Command::new("/usr/bin/open");
        if log_path.is_file() {
            command.args(["-a", "Console"]).arg(&log_path);
        } else {
            command.arg(log_path.parent().unwrap_or(&log_path));
        }
        // Sync commands run on the main thread: do not wait for `open` there.
        command
            .spawn()
            .map(|mut child| {
                std::thread::spawn(move || child.wait());
            })
            .map_err(|_| BackendError::new("LOG_CONSOLE_FAILED", "Could not open log console"))
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = app;
        Err(BackendError::new(
            "LOG_CONSOLE_FAILED",
            "Log console is only available on Windows",
        ))
    }
}
