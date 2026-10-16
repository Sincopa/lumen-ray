//! Core output is untrusted and can contain credentials and browsing destinations.
//! Persist only fixed diagnostic categories, never the original line or its fragments.
use tokio::io::{AsyncRead, AsyncReadExt};

fn contains_ignore_ascii_case(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() {
        return true;
    }
    haystack
        .windows(needle.len())
        .any(|window| window.eq_ignore_ascii_case(needle))
}

pub fn category(line: &[u8]) -> Option<&'static str> {
    let has = |needle: &[u8]| contains_ignore_ascii_case(line, needle);
    if has(b"unable to set routes") {
        Some("Windows tunnel route setup failed")
    } else if has(b"unable to set ips") {
        Some("Windows tunnel address setup failed")
    } else if has(b"unable to set dns") {
        Some("Windows tunnel DNS setup failed")
    } else if has(b"interface configuration failed") {
        Some("Windows tunnel interface is not ready")
    } else if has(b"wintun")
        || (has(b"failed to start proxy") && has(b"system cannot find the file specified"))
    {
        Some("Wintun adapter startup failed")
    } else if has(b"address already in use") || has(b"only one usage") {
        Some("local port already in use")
    } else if has(b"failed to start") {
        Some("startup failure")
    } else if has(b"certificate") {
        Some("TLS certificate error")
    } else if has(b"timeout") || has(b"deadline exceeded") {
        Some("network timeout")
    } else if has(b"refused") {
        Some("connection refused")
    } else if has(b"[error]") {
        Some("core error (details withheld)")
    } else if has(b"[warning]") {
        Some("core warning (details withheld)")
    } else {
        None
    }
}

pub struct CoreLogs {
    task: tokio::task::JoinHandle<()>,
    last_category: std::sync::Arc<std::sync::Mutex<Option<&'static str>>>,
}
impl Drop for CoreLogs {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl CoreLogs {
    pub fn from_reader(reader: impl AsyncRead + Unpin + Send + 'static) -> Self {
        let last_category = std::sync::Arc::new(std::sync::Mutex::new(None));
        Self {
            task: tokio::spawn(drain(reader, last_category.clone())),
            last_category,
        }
    }
    pub fn last_category(&self) -> Option<&'static str> {
        self.last_category.lock().ok().and_then(|value| *value)
    }
}
async fn drain(
    mut output: impl AsyncRead + Unpin,
    last_category: std::sync::Arc<std::sync::Mutex<Option<&'static str>>>,
) {
    let mut buffer = [0u8; 4096];
    let mut line = Vec::with_capacity(4096);
    let mut last = std::time::Instant::now() - std::time::Duration::from_secs(1);
    loop {
        let count = match output.read(&mut buffer).await {
            Ok(0) | Err(_) => break,
            Ok(count) => count,
        };
        for &byte in &buffer[..count] {
            if byte == b'\n' {
                if let Some(cat) = category(&line) {
                    if let Ok(mut value) = last_category.lock() {
                        *value = Some(cat);
                    }
                    if last.elapsed().as_secs() >= 1 {
                        log::warn!("Xray: {cat}");
                        last = std::time::Instant::now();
                    }
                }
                line.clear();
            } else if line.len() < 4096 {
                line.push(byte);
            }
        }
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn never_retains_original_output() {
        for message in [
            "[Warning] https://secret.invalid/token password=secret",
            "[Error] uuid-secret",
            "Failed to start: private subscription",
        ] {
            let value = super::category(message.as_bytes()).unwrap();
            assert!(
                !value.contains("secret") && !value.contains("private") && !value.contains("https")
            );
        }
        assert_eq!(super::category(b"uuid-private-key"), None);
    }
}
