//! Several Lumen copies can live on one PC: the installed one and portable builds
//! of other versions. They share one data directory, so only one runs at a time,
//! and it is the newest. A launch hands over to the newest copy Lumen has seen,
//! and a newer launch replaces an older running instance instead of focusing it.
use std::{
    ffi::c_void,
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0},
    Storage::FileSystem::{
        GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW, VS_FIXEDFILEINFO,
    },
    System::Threading::{
        CreateEventW, OpenEventW, OpenProcess, QueryFullProcessImageNameW, SetEvent,
        TerminateProcess, WaitForSingleObject, EVENT_MODIFY_STATE, INFINITE,
        PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE,
    },
    UI::WindowsAndMessaging::{FindWindowW, GetWindowThreadProcessId},
};

/// Set on a copy that was started by an older one, so it never hands over again.
const HANDED_OFF: &str = "LUMEN_HANDED_OFF";
/// Executables of every copy that has run, newest or not.
const REGISTRY: &str = "installations.json";
/// Graceful shutdown disconnects first; the exit path itself allows 25 s.
const GRACEFUL_EXIT_MS: u32 = 30_000;

type Version = [u16; 4];

struct Handle(HANDLE);
// A kernel handle may be used and closed from any thread.
unsafe impl Send for Handle {}
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

fn wide(text: &str) -> Vec<u16> {
    std::ffi::OsStr::new(text)
        .encode_wide()
        .chain(Some(0))
        .collect()
}

fn handoff_event(identifier: &str) -> Vec<u16> {
    wide(&format!("Local\\{identifier}-handoff"))
}

/// File version from the executable's resources; tauri-build writes the app version there.
fn file_version(path: &Path) -> Option<Version> {
    let name: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe {
        let size = GetFileVersionInfoSizeW(name.as_ptr(), std::ptr::null_mut());
        if size == 0 {
            return None;
        }
        let mut data = vec![0u8; size as usize];
        if GetFileVersionInfoW(name.as_ptr(), 0, size, data.as_mut_ptr().cast()) == 0 {
            return None;
        }
        let mut info: *mut c_void = std::ptr::null_mut();
        let mut length = 0u32;
        let root = wide("\\");
        if VerQueryValueW(data.as_ptr().cast(), root.as_ptr(), &mut info, &mut length) == 0
            || info.is_null()
            || (length as usize) < std::mem::size_of::<VS_FIXEDFILEINFO>()
        {
            return None;
        }
        let info = &*(info as *const VS_FIXEDFILEINFO);
        Some([
            (info.dwFileVersionMS >> 16) as u16,
            info.dwFileVersionMS as u16,
            (info.dwFileVersionLS >> 16) as u16,
            info.dwFileVersionLS as u16,
        ])
    }
}

fn package_version() -> Version {
    let mut parts = env!("CARGO_PKG_VERSION")
        .split(['.', '-', '+'])
        .map(|part| part.parse().unwrap_or(0));
    [
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        0,
    ]
}

fn same_file(left: &Path, right: &Path) -> bool {
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

/// Records this copy and returns the known copies that still exist.
fn register(data: &Path, current: &Path) -> Vec<PathBuf> {
    let path = data.join(REGISTRY);
    let mut known: Vec<PathBuf> = std::fs::read(&path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    known.retain(|copy| copy.is_file() && !same_file(copy, current));
    known.push(current.to_path_buf());
    if std::fs::create_dir_all(data).is_ok() {
        if let Ok(bytes) = serde_json::to_vec(&known) {
            let _ = std::fs::write(&path, bytes);
        }
    }
    known
}

/// Process id of the running instance, from the single-instance plugin's window.
fn running_instance(identifier: &str) -> Option<u32> {
    let class = wide(&format!("{identifier}-sic"));
    let title = wide(&format!("{identifier}-siw"));
    unsafe {
        let window = FindWindowW(class.as_ptr(), title.as_ptr());
        if window.is_null() {
            return None;
        }
        let mut pid = 0;
        GetWindowThreadProcessId(window, &mut pid);
        (pid != 0 && pid != std::process::id()).then_some(pid)
    }
}

fn process_path(process: &Handle) -> Option<PathBuf> {
    let mut path = vec![0u16; 32768];
    let mut size = path.len() as u32;
    unsafe {
        if QueryFullProcessImageNameW(process.0, 0, path.as_mut_ptr(), &mut size) == 0 {
            return None;
        }
    }
    Some(PathBuf::from(String::from_utf16_lossy(
        &path[..size as usize],
    )))
}

/// Closes an older running instance. Versions with the handoff event disconnect
/// and restore the system proxy themselves; older ones are ended, and the
/// network journal is recovered when this instance opens its state.
fn replace_older(identifier: &str, pid: u32, version: Version) {
    let process = unsafe {
        OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE | PROCESS_TERMINATE,
            0,
            pid,
        )
    };
    if process.is_null() {
        // An elevated instance cannot be closed from here; it keeps the focus.
        return;
    }
    let process = Handle(process);
    let Some(running) = process_path(&process).and_then(|path| file_version(&path)) else {
        return;
    };
    if running >= version {
        return;
    }
    let name = handoff_event(identifier);
    let event = unsafe { OpenEventW(EVENT_MODIFY_STATE, 0, name.as_ptr()) };
    let wait = if event.is_null() {
        0
    } else {
        let event = Handle(event);
        unsafe { SetEvent(event.0) };
        GRACEFUL_EXIT_MS
    };
    log::info!("Replacing an older running Lumen {running:?} with {version:?}");
    unsafe {
        if WaitForSingleObject(process.0, wait) != WAIT_OBJECT_0 {
            TerminateProcess(process.0, 0);
            WaitForSingleObject(process.0, 5_000);
        }
    }
}

/// Runs before Tauri starts. Either returns to start this copy, or exits after
/// the newest copy was started in its place.
pub fn launch_newest(identifier: &str) {
    // A development build must never take over an installed Lumen.
    if cfg!(debug_assertions) {
        return;
    }
    let (Some(local), Ok(current)) = (std::env::var_os("LOCALAPPDATA"), std::env::current_exe())
    else {
        return;
    };
    let data = PathBuf::from(local).join(identifier);
    let known = register(&data, &current);
    let version = file_version(&current).unwrap_or_else(package_version);
    if std::env::var_os(HANDED_OFF).is_none() {
        let newest = known
            .iter()
            .filter(|copy| !same_file(copy, &current))
            .filter_map(|copy| Some((file_version(copy)?, copy)))
            .filter(|(candidate, _)| *candidate > version)
            .max_by_key(|(candidate, _)| *candidate);
        if let Some((_, newest)) = newest {
            let started = std::process::Command::new(newest)
                .args(std::env::args_os().skip(1))
                .env(HANDED_OFF, "1")
                .spawn();
            if started.is_ok() {
                std::process::exit(0);
            }
        }
    }
    if let Some(pid) = running_instance(identifier) {
        replace_older(identifier, pid, version);
    }
}

/// Lets a newer copy ask this instance to exit through the normal cleanup path.
pub fn listen_for_handoff(app: tauri::AppHandle, identifier: &str) {
    let name = handoff_event(identifier);
    let event = unsafe { CreateEventW(std::ptr::null(), 1, 0, name.as_ptr()) };
    if event.is_null() {
        return;
    }
    let event = Handle(event);
    std::thread::spawn(move || {
        let event = event;
        if unsafe { WaitForSingleObject(event.0, INFINITE) } == WAIT_OBJECT_0 {
            log::info!("A newer Lumen started; exiting so it can take over");
            app.exit(0);
        }
    });
}
