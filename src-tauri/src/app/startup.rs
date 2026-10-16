//! Startup diagnostics must work before Tauri (and therefore WebView2) starts.
//! Reports contain categories only: never subscriptions, arguments or panic payloads.
use std::sync::atomic::{AtomicBool, Ordering};
use std::{
    io::Write,
    path::{Path, PathBuf},
};
use tauri::Manager;

#[derive(Default)]
pub struct FrontendReadiness(AtomicBool);

#[tauri::command]
pub fn frontend_ready(readiness: tauri::State<'_, FrontendReadiness>) {
    if !readiness.0.swap(true, Ordering::SeqCst) {
        record("FRONTEND_READY");
    }
}

pub fn watch_frontend(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(45)).await;
        if !app.state::<FrontendReadiness>().0.load(Ordering::SeqCst) {
            // A Windows dialog remains usable if the WebView could not load the UI.
            let _ = tauri::async_runtime::spawn_blocking(|| {
                report(classify("FRONTEND_LOAD_FAILED"));
            })
            .await;
        }
    });
}

pub fn verify_embedded_frontend(context: &tauri::Context<tauri::Wry>) -> Result<(), Failure> {
    if cfg!(dev) || context.assets().get(&"index.html".into()).is_none() {
        return Err(classify("FRONTEND_NOT_EMBEDDED"));
    }
    record("EMBEDDED_FRONTEND_OK");
    Ok(())
}

pub const WEBVIEW_DOWNLOAD: &str = "https://developer.microsoft.com/microsoft-edge/webview2/";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Failure {
    pub code: &'static str,
    pub advice: &'static str,
}

pub fn classify(message: &str) -> Failure {
    let (code, advice) = if message.contains("CORE_NOT_FOUND") {
        ("CORE_NOT_FOUND", "Не найдены или повреждены файлы Xray/Wintun. Распакуйте весь portable-архив в отдельную папку: рядом с Lumen.exe должна быть папка runtime. Проверьте, не переместил ли антивирус файлы в карантин.")
    } else if message.contains("GEO_UPDATE_FAILED") {
        ("GEO_UPDATE_FAILED", "Не удалось прочитать или проверить базы GeoIP/GeoSite. Распакуйте полный архив заново и проверьте доступ к папке данных Lumen.")
    } else if message.contains("STORAGE_VERSION") {
        (
            "STORAGE_VERSION",
            "Данные сохранены более новой версией Lumen. Установите последнюю версию приложения.",
        )
    } else if message.contains("STORAGE_FAILED") || message.contains("SECRET_STORE_FAILED") {
        ("STORAGE_FAILED", "Не удалось прочитать или сохранить данные Lumen. Проверьте свободное место и доступ к %LOCALAPPDATA%\\app.lumen.desktop. Зашифрованные данные нельзя переносить из другого аккаунта Windows.")
    } else if message.contains("ALREADY_RUNNING") {
        ("ALREADY_RUNNING", "Lumen уже запущен. Откройте его через значок в системном трее. Если окно не появляется, завершите только процесс Lumen и повторите запуск.")
    } else if message.contains("WEBVIEW2_UNAVAILABLE") {
        ("WEBVIEW2_UNAVAILABLE", "Microsoft Edge WebView2 Runtime не найден или недоступен. Установите или восстановите Evergreen Runtime с сайта Microsoft, затем запустите Lumen снова.")
    } else if message.contains("WEBVIEW_INIT_FAILED") {
        ("WEBVIEW_INIT_FAILED", "Не удалось создать окно WebView2. Восстановите WebView2 Runtime и проверьте доступ к папке данных Lumen. Если ошибка повторяется, отправьте журнал запуска разработчику.")
    } else if message.contains("FRONTEND_NOT_EMBEDDED") {
        ("FRONTEND_NOT_EMBEDDED", "В этой сборке отсутствует встроенный интерфейс Lumen. Скачайте исправленную portable-версию приложения.")
    } else if message.contains("FRONTEND_LOAD_FAILED") {
        ("FRONTEND_LOAD_FAILED", "Интерфейс Lumen не загрузился. Перезапустите приложение. Если ошибка повторяется, восстановите WebView2 Runtime и отправьте журнал запуска разработчику.")
    } else if message.contains("APPLICATION_PANIC") {
        ("APPLICATION_PANIC", "В приложении произошла внутренняя ошибка. Повторите запуск. Если ошибка повторяется, отправьте журнал запуска разработчику.")
    } else {
        ("INITIALIZATION_FAILED", "Не удалось инициализировать Lumen. Распакуйте полный portable-архив заново и запустите Lumen.exe. Если ошибка повторяется, отправьте журнал запуска разработчику.")
    };
    Failure { code, advice }
}

fn log_candidates() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        paths.push(PathBuf::from(local).join("app.lumen.desktop/logs/startup.log"));
    }
    paths.push(std::env::temp_dir().join("lumen-startup.log"));
    paths
}

fn append_report(path: &Path, event: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let rotate = std::fs::metadata(path).is_ok_and(|m| m.len() > 512 * 1024);
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(!rotate)
        .truncate(rotate)
        .open(path)?;
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    writeln!(
        file,
        "timestamp={timestamp} version={} arch={} pid={} {event}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::ARCH,
        std::process::id(),
    )
}

pub fn record(event: &str) -> Option<PathBuf> {
    log_candidates()
        .into_iter()
        .find(|path| append_report(path, event).is_ok())
}

pub fn report(failure: Failure) {
    let path = record(failure.code);
    let log = path
        .map(|path| format!("\n\nЖурнал запуска: {}", path.display()))
        .unwrap_or_else(|| "\n\nНе удалось записать журнал. Проверьте доступ к папке временных файлов и свободное место.".into());
    let mut message = format!(
        "Lumen не удалось запустить.\nКод: {}\n\n{}{}",
        failure.code, failure.advice, log,
    );
    let download = failure.code == "WEBVIEW2_UNAVAILABLE";
    if download {
        message.push_str("\n\nОткрыть страницу загрузки WebView2 на сайте Microsoft?");
    }
    show_dialog(&message, download);
}

#[cfg(windows)]
fn show_dialog(message: &str, download: bool) {
    use windows_sys::Win32::UI::{
        Shell::ShellExecuteW,
        WindowsAndMessaging::{MessageBoxW, IDYES, MB_ICONERROR, MB_OK, MB_YESNO},
    };
    let wide = |value: &str| value.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    let body = wide(message);
    let title = wide("Lumen — ошибка запуска");
    // A native Windows dialog works even when no WebView2 or Tauri window exists.
    let result = unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            body.as_ptr(),
            title.as_ptr(),
            MB_ICONERROR | if download { MB_YESNO } else { MB_OK },
        )
    };
    if download && result == IDYES {
        let url = wide(WEBVIEW_DOWNLOAD);
        let verb = wide("open");
        unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                verb.as_ptr(),
                url.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1,
            );
        }
    }
}

#[cfg(not(windows))]
fn show_dialog(message: &str, _download: bool) {
    eprintln!("{message}");
}

pub fn install_panic_handler() {
    std::panic::set_hook(Box::new(|info| {
        // The payload may contain a subscription URL or credentials. Do not log it.
        if let Some(location) = info.location() {
            record(&format!(
                "APPLICATION_PANIC source={} line={}",
                Path::new(location.file())
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy(),
                location.line(),
            ));
        }
        report(classify("APPLICATION_PANIC"));
    }));
}

pub fn preflight() -> Result<(), Failure> {
    record("PREFLIGHT_STARTED");
    #[cfg(windows)]
    {
        match tauri::webview_version() {
            Ok(version) if !version.trim().is_empty() => {
                // Only the numeric version, never loader-provided arbitrary text.
                let version: String = version
                    .chars()
                    .filter(|c| c.is_ascii_digit() || *c == '.')
                    .collect();
                record(&format!("WEBVIEW2_AVAILABLE version={version}"));
            }
            _ => return Err(classify("WEBVIEW2_UNAVAILABLE")),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_errors_have_specific_recovery_advice() {
        for code in [
            "CORE_NOT_FOUND",
            "GEO_UPDATE_FAILED",
            "STORAGE_VERSION",
            "STORAGE_FAILED",
            "ALREADY_RUNNING",
            "WEBVIEW2_UNAVAILABLE",
            "WEBVIEW_INIT_FAILED",
            "APPLICATION_PANIC",
            "FRONTEND_NOT_EMBEDDED",
            "FRONTEND_LOAD_FAILED",
        ] {
            let failure = classify(&format!(
                "error encountered during setup hook: {code}: detail"
            ));
            assert_eq!(failure.code, code);
            assert!(!failure.advice.is_empty());
        }
    }

    #[test]
    fn unknown_errors_never_disclose_their_payload() {
        let input = "plugin failed with https://secret.example/sub/token and password";
        let failure = classify(input);
        assert_eq!(failure.code, "INITIALIZATION_FAILED");
        assert!(!failure.advice.contains("secret.example"));
        assert!(!failure.advice.contains("password"));
    }

    #[test]
    fn startup_report_is_bounded_and_can_precede_app_data() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("logs/startup.log");
        append_report(&log, "PREFLIGHT_STARTED").unwrap();
        append_report(&log, "CORE_NOT_FOUND").unwrap();
        let text = std::fs::read_to_string(&log).unwrap();
        assert!(text.contains("version="));
        assert!(text.contains("PREFLIGHT_STARTED"));
        assert!(text.contains("CORE_NOT_FOUND"));
        std::fs::write(&log, vec![b'x'; 512 * 1024 + 1]).unwrap();
        append_report(&log, "PREFLIGHT_STARTED").unwrap();
        assert!(std::fs::metadata(&log).unwrap().len() < 1024);
    }
}
