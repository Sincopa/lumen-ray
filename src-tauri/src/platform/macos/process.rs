//! No Job Objects on macOS: each core leaves a record so the next launch can stop an orphan.
use crate::domain::errors::{BackendError, Result};
use std::{
    os::{fd::OwnedFd, unix::process::CommandExt},
    path::{Path, PathBuf},
    process::{Child, Stdio},
    sync::{Mutex, OnceLock},
    time::Duration,
};
use tokio::io::AsyncWriteExt;

static RECORDS: OnceLock<PathBuf> = OnceLock::new();

pub struct CoreProcess {
    child: Mutex<Child>,
    pid: i32,
    record: Option<PathBuf>,
    _logs: Option<crate::xray::logs::CoreLogs>,
}
fn error() -> BackendError {
    BackendError::new("CORE_START_FAILED", "Could not launch managed Xray process")
}
/// Start time identifies a pid across reuse; a recycled pid never matches it.
pub(crate) fn started(pid: i32) -> Option<(u64, u64)> {
    let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of_val(&info) as libc::c_int;
    let read = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTBSDINFO,
            0,
            (&mut info as *mut libc::proc_bsdinfo).cast(),
            size,
        )
    };
    (read == size).then_some((info.pbi_start_tvsec, info.pbi_start_tvusec))
}
pub(crate) fn executable(pid: i32) -> Option<PathBuf> {
    let mut buffer = vec![0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
    let len = unsafe { libc::proc_pidpath(pid, buffer.as_mut_ptr().cast(), buffer.len() as u32) };
    if len <= 0 {
        return None;
    }
    buffer.truncate(len as usize);
    use std::os::unix::ffi::OsStringExt;
    Some(PathBuf::from(std::ffi::OsString::from_vec(buffer)))
}
fn record_text(start: (u64, u64), path: &Path) -> String {
    format!("{} {}\n{}", start.0, start.1, path.display())
}
fn parse_record(text: &str) -> Option<((u64, u64), PathBuf)> {
    let (start, path) = text.split_once('\n')?;
    let (sec, usec) = start.split_once(' ')?;
    (!path.is_empty()).then_some(())?;
    Some(((sec.parse().ok()?, usec.parse().ok()?), PathBuf::from(path)))
}
impl CoreProcess {
    #[allow(dead_code)]
    pub fn diagnostic_category(&self) -> Option<&'static str> {
        self._logs
            .as_ref()
            .and_then(crate::xray::logs::CoreLogs::last_category)
    }
    pub async fn spawn(core: &Path, config: &[u8]) -> Result<Self> {
        let (output, writer) = std::io::pipe().map_err(|_| error())?;
        let mut command = std::process::Command::new(core);
        // Each child gets one immutable geo snapshot; do not race through process-global env.
        command
            .args(["run", "-config", "stdin:"])
            .current_dir(core.parent().ok_or_else(error)?)
            .env(
                "XRAY_LOCATION_ASSET",
                crate::xray::geo::assets_for_core(core),
            )
            .envs(crate::engine::xray::GO_RUNTIME_ENV)
            .stdin(Stdio::piped())
            .stdout(writer.try_clone().map_err(|_| error())?)
            .stderr(writer)
            // A terminal Ctrl-C or a signal to Lumen's group must not reach the core.
            .process_group(0);
        let mut child = command.spawn().map_err(|_| error())?;
        // The parent's copies of the write end must close, or the log reader never sees EOF.
        drop(command);
        let pid = child.id() as i32;
        let input = child.stdin.take();
        let record = RECORDS.get().and_then(|directory| {
            let path = directory.join(format!("{pid}.core"));
            // The kernel's path, not ours, so symlinks in the data dir still compare equal.
            std::fs::write(&path, record_text(started(pid)?, &executable(pid)?))
                .ok()
                .map(|()| path)
        });
        let process = Self {
            child: Mutex::new(child),
            pid,
            record,
            _logs: Some(crate::xray::logs::CoreLogs::from_reader(
                tokio::fs::File::from_std(std::fs::File::from(OwnedFd::from(output))),
            )),
        };
        let mut input =
            tokio::fs::File::from_std(std::fs::File::from(OwnedFd::from(input.ok_or_else(error)?)));
        tokio::time::timeout(Duration::from_secs(10), input.write_all(config))
            .await
            .map_err(|_| error())?
            .map_err(|_| error())?;
        input.flush().await.map_err(|_| error())?;
        drop(input);
        Ok(process)
    }
    pub fn try_wait(&self) -> std::io::Result<Option<std::process::ExitStatus>> {
        let exit = self
            .child
            .lock()
            .map_err(|_| std::io::Error::other("Core process lock poisoned"))?
            .try_wait()?;
        if exit.is_some() {
            self.forget();
        }
        Ok(exit)
    }
    pub async fn wait(&mut self) -> std::io::Result<std::process::ExitStatus> {
        loop {
            if let Some(exit) = self.try_wait()? {
                return Ok(exit);
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
    pub async fn kill(&mut self) -> std::io::Result<()> {
        if self.try_wait()?.is_some() {
            return Ok(());
        }
        // Not reaped yet, so the pid and group cannot have been reused.
        if unsafe { libc::killpg(self.pid, libc::SIGTERM) } == 0 {
            if let Ok(exit) = tokio::time::timeout(Duration::from_secs(3), self.wait()).await {
                let exit = exit?;
                log::info!("Xray graceful shutdown: {exit}");
                return Ok(());
            }
        }
        log::warn!("Xray shutdown timed out; terminating owned process");
        if unsafe { libc::killpg(self.pid, libc::SIGKILL) } != 0 && self.try_wait()?.is_none() {
            return Err(std::io::Error::last_os_error());
        }
        tokio::time::timeout(Duration::from_secs(3), self.wait())
            .await
            .map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::TimedOut, "Core termination timed out")
            })??;
        Ok(())
    }
    fn forget(&self) {
        if let Some(record) = &self.record {
            let _ = std::fs::remove_file(record);
        }
    }
}
impl Drop for CoreProcess {
    // Matches JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: a dropped handle never leaves a core running.
    fn drop(&mut self) {
        if let Ok(child) = self.child.get_mut() {
            if matches!(child.try_wait(), Ok(None)) {
                unsafe {
                    libc::killpg(self.pid, libc::SIGKILL);
                }
                let _ = child.wait();
            }
        }
        self.forget();
    }
}
/// Stops cores orphaned by a crashed Lumen and enables records for new ones.
pub fn recover(directory: &Path) {
    stop_recorded(directory);
    track(directory);
}
/// Records new cores without stopping old ones: a core that macOS proxies still
/// point at is kept while they cannot be restored.
pub fn track(directory: &Path) {
    let directory = directory.join("cores");
    if std::fs::create_dir_all(&directory).is_err() {
        log::warn!("Core process records are unavailable");
        return;
    }
    let _ = RECORDS.set(directory);
}
/// Only a process that still has the recorded start time and executable is ours.
/// Exit uses it too: a core held by an in-flight connect is never dropped.
pub fn stop_recorded(directory: &Path) {
    let Ok(entries) = std::fs::read_dir(directory.join("cores")) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let pid = path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_suffix(".core"))
            .and_then(|pid| pid.parse::<i32>().ok())
            .filter(|pid| *pid > 1);
        let record = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| parse_record(&text));
        if let (Some(pid), Some((start, core))) = (pid, record) {
            if started(pid) == Some(start) && executable(pid).as_deref() == Some(&*core) {
                // The core leads its own group unless something replaced it.
                let group = unsafe { libc::getpgid(pid) } == pid;
                unsafe {
                    if group {
                        libc::killpg(pid, libc::SIGKILL);
                    } else {
                        libc::kill(pid, libc::SIGKILL);
                    }
                }
                log::warn!("Stopped a recorded Xray core");
            }
        }
        let _ = std::fs::remove_file(&path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_records_round_trip_and_reject_damage() {
        let core = Path::new("/Users/a b/Library/Application Support/app.lumen.desktop/xray");
        let text = record_text((1_700_000_000, 42), core);
        assert_eq!(
            parse_record(&text),
            Some(((1_700_000_000, 42), core.into()))
        );
        for bad in ["", "1 2", "1 2\n", "x 2\n/xray", "1\n/xray", "1 2 3\n/xray"] {
            assert_eq!(parse_record(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn identity_of_this_process_is_readable() {
        let pid = std::process::id() as i32;
        assert!(started(pid).is_some());
        assert_eq!(
            executable(pid).and_then(|path| path.canonicalize().ok()),
            std::env::current_exe()
                .and_then(|path| path.canonicalize())
                .ok()
        );
        assert_eq!(started(-1), None);
    }
}
