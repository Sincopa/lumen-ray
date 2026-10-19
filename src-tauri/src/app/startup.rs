//! Startup diagnostics must work before Tauri (and therefore WebView2) starts.
//! Reports contain categories only: never subscriptions or arguments. Panic
//! messages are included only after `redact_panic_message` strips URLs,
//! addresses and long tokens.
use std::sync::atomic::{AtomicBool, Ordering};

/// Set once the UI has loaded: later panics are runtime failures, not startup ones.
static LAUNCHED: AtomicBool = AtomicBool::new(false);
/// A panicking loop must not stack up dialogs.
static PANIC_REPORTED: AtomicBool = AtomicBool::new(false);
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
        LAUNCHED.store(true, Ordering::SeqCst);
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
    show_dialog("Lumen — ошибка запуска", &message, download);
}

#[cfg(windows)]
fn show_dialog(title: &str, message: &str, download: bool) {
    use windows_sys::Win32::UI::{
        Shell::ShellExecuteW,
        WindowsAndMessaging::{MessageBoxW, IDYES, MB_ICONERROR, MB_OK, MB_YESNO},
    };
    let wide = |value: &str| value.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    let body = wide(message);
    let title = wide(title);
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
fn show_dialog(title: &str, message: &str, _download: bool) {
    eprintln!("{title}\n{message}");
}

/// Keeps the words of a panic message that cannot carry user data. A panic
/// payload may embed a subscription URL, server address or key, so URLs,
/// anything with `@`, and long tokens (UUIDs, keys, base64) are replaced.
pub fn redact_panic_message(message: &str) -> String {
    let mut text = message
        .split_whitespace()
        .map(|word| {
            if word.contains("://") || word.contains('@') || word.chars().count() > 32 {
                "[скрыто]"
            } else {
                word
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    if let Some((cut, _)) = text.char_indices().nth(200) {
        text.truncate(cut);
        text.push('…');
    }
    text
}

pub fn install_panic_handler() {
    std::panic::set_hook(Box::new(|info| {
        let place = info.location().map(|location| {
            format!(
                "{}:{}",
                Path::new(location.file())
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy(),
                location.line(),
            )
        });
        let reason = info
            .payload_as_str()
            .map(redact_panic_message)
            .filter(|reason| !reason.is_empty());
        let thread = std::thread::current()
            .name()
            .unwrap_or("unnamed")
            .to_owned();
        let launched = LAUNCHED.load(Ordering::SeqCst);
        let path = record(&format!(
            "APPLICATION_PANIC phase={} source={} thread={thread} reason={}",
            if launched { "runtime" } else { "startup" },
            place.as_deref().unwrap_or("unknown"),
            reason.as_deref().unwrap_or("unknown"),
        ));
        if PANIC_REPORTED.swap(true, Ordering::SeqCst) {
            return;
        }
        let mut details = format!("Место: {}", place.as_deref().unwrap_or("неизвестно"));
        if let Some(reason) = &reason {
            details.push_str(&format!("\nПричина: {reason}"));
        }
        let log = path
            .map(|path| format!("\n\nЖурнал: {}", path.display()))
            .unwrap_or_default();
        let (title, message) = if launched {
            (
                "Lumen — внутренняя ошибка",
                format!(
                    "Lumen работает, но его фоновая служба остановилась из-за ошибки в программе.\nКод: APPLICATION_PANIC\n{details}\n\nVPN может перестать пропускать трафик или переподключаться. Отключите VPN и перезапустите Lumen. Отправьте этот текст и журнал разработчику.{log}"
                ),
            )
        } else {
            (
                "Lumen — ошибка запуска",
                format!(
                    "Lumen не удалось запустить.\nКод: APPLICATION_PANIC\n{details}\n\n{}{log}",
                    classify("APPLICATION_PANIC").advice,
                ),
            )
        };
        if launched {
            // The panicking thread may be an async worker: a modal dialog on it
            // would stall unrelated tasks until the user presses OK.
            let _ = std::thread::Builder::new()
                .name("lumen-panic-dialog".into())
                .spawn(move || show_dialog(title, &message, false));
        } else {
            show_dialog(title, &message, false);
        }
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
    fn panic_messages_keep_the_reason_but_not_user_data() {
        assert_eq!(
            redact_panic_message("overflow when subtracting duration from instant"),
            "overflow when subtracting duration from instant"
        );
        let redacted = redact_panic_message(
            "bad node vless://0b1c2d3e-4f50-6172-8394-a5b6c7d8e9f0@vpn.example:443 key=QUJDREVGR0hJSktMTU5PUFFSU1RVVldYWVo0123456789",
        );
        assert!(redacted.starts_with("bad node [скрыто]"));
        assert!(!redacted.contains("vpn.example"));
        assert!(!redacted.contains("QUJD"));
        assert!(redact_panic_message(&"слово ".repeat(100)).chars().count() <= 201);
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
