//! Log records of the elevated tunnel helper.
//!
//! The helper writes no log file of its own: an elevated process writing into
//! the user's profile could be redirected by whoever replaces that file with a
//! link. Its records travel over a second pipe to the parent, which owns the
//! pipe and writes them into the normal application log (Lumen.log).
//!
//! Records carry only Lumen's own fixed messages and Xray diagnostic categories,
//! never Xray output, server addresses or credentials.
use std::{
    io::Write,
    os::windows::{fs::OpenOptionsExt, io::AsRawHandle},
    sync::{mpsc, Mutex},
    time::{Duration, Instant},
};
use tokio::{io::AsyncReadExt, net::windows::named_pipe::NamedPipeServer};
use windows_sys::Win32::{
    Storage::FileSystem::{SECURITY_IDENTIFICATION, SECURITY_SQOS_PRESENT},
    System::Pipes::{GetNamedPipeClientProcessId, GetNamedPipeServerProcessId},
};

/// Longest record kept; the rest of a longer one is dropped.
const MAX_LINE: usize = 512;
/// Records waiting for the writer thread. Beyond this they are dropped, so a
/// parent that stops reading can never stall the helper.
const QUEUE: usize = 256;
/// The parent's log target. Lumen.log keeps `app_lib` targets only.
const TARGET: &str = "app_lib::tun_helper";
/// The same record inside this window is counted, not sent again: a core that
/// keeps failing to restart must not push useful history out of the rotated log.
const REPEAT_WINDOW: Duration = Duration::from_secs(30);
/// Distinct recent records remembered for that.
const RECENT: usize = 64;

pub fn pipe_name(id: &str) -> String {
    format!(r"\\.\pipe\lumen-{id}-log")
}

/// Printable text only, on one line, at most `MAX_LINE` bytes.
fn sanitize(text: &str) -> String {
    let mut line = String::with_capacity(text.len().min(MAX_LINE));
    for c in text.chars() {
        let c = if c.is_control() { ' ' } else { c };
        if line.len() + c.len_utf8() > MAX_LINE {
            break;
        }
        line.push(c);
    }
    line
}

// Helper side ----------------------------------------------------------------

struct PipeLogger {
    sender: Mutex<Option<mpsc::SyncSender<String>>>,
    /// Recent records: text, when it was last sent, repeats held back since.
    recent: Mutex<Vec<(String, Instant, u32)>>,
}

impl PipeLogger {
    /// The record to send now, or None while it repeats inside the window.
    fn admit(&self, message: String, now: Instant) -> Option<String> {
        let Ok(mut recent) = self.recent.lock() else {
            return Some(message);
        };
        if let Some(entry) = recent.iter_mut().find(|entry| entry.0 == message) {
            if now.duration_since(entry.1) < REPEAT_WINDOW {
                entry.2 += 1;
                return None;
            }
            entry.1 = now;
            let repeated = std::mem::take(&mut entry.2);
            return Some(if repeated > 0 {
                format!("{message} (and {repeated} more time(s) before)")
            } else {
                message
            });
        }
        if recent.len() >= RECENT {
            recent.remove(0);
        }
        recent.push((message.clone(), now, 0));
        Some(message)
    }
}

impl log::Log for PipeLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::Level::Info && metadata.target().starts_with("app_lib")
    }
    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let level = match record.level() {
            log::Level::Error => 'E',
            log::Level::Warn => 'W',
            _ => 'I',
        };
        let Some(message) = self.admit(record.args().to_string(), Instant::now()) else {
            return;
        };
        let line = format!("{level}{}", sanitize(&message));
        if let Ok(sender) = self.sender.lock() {
            if let Some(sender) = sender.as_ref() {
                let _ = sender.try_send(line);
            }
        }
    }
    fn flush(&self) {}
}

static LOGGER: PipeLogger = PipeLogger {
    sender: Mutex::new(None),
    recent: Mutex::new(Vec::new()),
};
static WRITER: Mutex<Option<std::thread::JoinHandle<()>>> = Mutex::new(None);

/// Route the helper's records to the parent's log pipe. Without the pipe the
/// helper still runs, only without a log.
pub fn install(id: &str, parent: u32) {
    attach(id, parent);
    install_logger();
}

/// Make this process log through the pipe logger. Records are dropped while no
/// session is attached. Safe to call more than once.
pub fn install_logger() {
    static INSTALLED: std::sync::Once = std::sync::Once::new();
    INSTALLED.call_once(|| {
        if log::set_logger(&LOGGER).is_ok() {
            log::set_max_level(log::LevelFilter::Info);
        }
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let at = info
                .location()
                .map(|place| format!("{}:{}", place.file(), place.line()))
                .unwrap_or_default();
            log::error!("TUN helper panicked at {at}");
            previous(info);
        }));
    });
}

/// Send records to the log pipe of session `id`, which process `owner` must
/// have created. The tunnel service attaches once per Lumen session.
pub fn attach(id: &str, owner: u32) {
    let Ok(mut pipe) = std::fs::OpenOptions::new()
        .write(true)
        // The parent must not act with the elevated helper's identity.
        .security_qos_flags(SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION)
        .open(pipe_name(id))
    else {
        return;
    };
    let mut server = 0;
    let owned = unsafe {
        GetNamedPipeServerProcessId(pipe.as_raw_handle() as _, &mut server) != 0
            && server == owner
    };
    if !owned {
        return;
    }
    let (sender, receiver) = mpsc::sync_channel::<String>(QUEUE);
    let writer = std::thread::spawn(move || {
        for mut line in receiver {
            line.push('\n');
            if pipe.write_all(line.as_bytes()).is_err() {
                break;
            }
        }
    });
    if let Ok(mut slot) = LOGGER.sender.lock() {
        *slot = Some(sender);
    }
    if let Ok(mut slot) = WRITER.lock() {
        *slot = Some(writer);
    }
}

/// Deliver queued records, then stop sending (helper exit or service session end).
pub fn finish() {
    if let Ok(mut slot) = LOGGER.sender.lock() {
        slot.take();
    }
    let writer = WRITER.lock().ok().and_then(|mut slot| slot.take());
    if let Some(writer) = writer {
        let deadline = Instant::now() + Duration::from_millis(500);
        while !writer.is_finished() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

// Parent side ----------------------------------------------------------------

/// Write what the helper process `helper` sends into the application log,
/// until it closes the pipe. Any other client is ignored.
pub fn forward(mut pipe: NamedPipeServer, helper: u32) {
    tokio::spawn(async move {
        if tokio::time::timeout(Duration::from_secs(30), pipe.connect())
            .await
            .map_or(true, |connected| connected.is_err())
        {
            return;
        }
        let mut client = 0;
        let verified = unsafe {
            GetNamedPipeClientProcessId(pipe.as_raw_handle() as _, &mut client) != 0
                && client == helper
        };
        if !verified {
            return;
        }
        let mut buffer = [0u8; 4096];
        let mut line = Vec::with_capacity(MAX_LINE);
        loop {
            let count = match pipe.read(&mut buffer).await {
                Ok(0) | Err(_) => break,
                Ok(count) => count,
            };
            for &byte in &buffer[..count] {
                if byte == b'\n' {
                    emit(&line);
                    line.clear();
                } else if line.len() < MAX_LINE + 1 {
                    line.push(byte);
                }
            }
        }
        if !line.is_empty() {
            emit(&line);
        }
    });
}

fn emit(line: &[u8]) {
    let text = String::from_utf8_lossy(line);
    let mut chars = text.chars();
    let level = match chars.next() {
        Some('E') => log::Level::Error,
        Some('W') => log::Level::Warn,
        Some('I') => log::Level::Info,
        _ => return,
    };
    let message = sanitize(chars.as_str());
    if !message.is_empty() {
        log::log!(target: TARGET, level, "TUN helper: {message}");
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    #[test]
    fn repeats_inside_the_window_are_counted_not_sent() {
        let logger = super::PipeLogger {
            sender: std::sync::Mutex::new(None),
            recent: std::sync::Mutex::new(Vec::new()),
        };
        let start = Instant::now();
        let restart = || "Xray standby restart did not become ready".to_owned();
        assert_eq!(logger.admit(restart(), start), Some(restart()));
        assert_eq!(logger.admit(restart(), start + Duration::from_secs(1)), None);
        assert_eq!(logger.admit(restart(), start + Duration::from_secs(2)), None);
        // Other records still pass while one repeats.
        assert!(logger
            .admit("Routes activated".into(), start + Duration::from_secs(3))
            .is_some());
        assert_eq!(
            logger.admit(restart(), start + Duration::from_secs(31)),
            Some(format!("{} (and 2 more time(s) before)", restart()))
        );
    }

    #[test]
    fn records_stay_on_one_bounded_line() {
        let long = "x".repeat(2000);
        assert_eq!(super::sanitize(&long).len(), super::MAX_LINE);
        assert_eq!(super::sanitize("a\r\nb\tc"), "a  b c");
        // A multibyte character is never cut in half.
        let cyrillic = "ж".repeat(400);
        let kept = super::sanitize(&cyrillic);
        assert!(kept.len() <= super::MAX_LINE && kept.chars().all(|c| c == 'ж'));
    }
}
