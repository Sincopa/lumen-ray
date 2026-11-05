//! Lumen Tunnel service.
//!
//! The TUN adapter and routes need administrator rights. Lumen used to start an
//! elevated `--lumen-tunnel` helper through UAC for every app session, so each
//! launch asked for permission. This service runs the same helper session as
//! LocalSystem; the user approves it once, when Lumen installs it or after an
//! update replaces Lumen.exe.
//!
//! Trust:
//! - The service runs its own copy of Lumen.exe from Program Files, which a
//!   standard user cannot change, never the per-user installation.
//! - It serves a client whose executable is byte-identical to that copy: the
//!   same build, the same compiled-in Xray hashes and config builder. Any other
//!   build is told to reinstall the service.
//! - The session protocol is the helper's: structured settings and a verified
//!   geo snapshot. Executables taken from the client's runtime folder must match
//!   the compiled-in hashes and run from a folder only SYSTEM and administrators
//!   can write.
//! - Lumen sends its settings only to a pipe server in session 0, where only
//!   services run.
//! - Demand start; the service stops itself after a few idle minutes.
use crate::domain::errors::{BackendError, Result};
use sha2::{Digest, Sha256};
use std::{
    os::windows::{
        ffi::OsStrExt,
        fs::OpenOptionsExt,
        io::{AsRawHandle, IntoRawHandle},
    },
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering},
        Arc, OnceLock,
    },
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::windows::named_pipe::{NamedPipeClient, NamedPipeServer, ServerOptions},
    sync::watch,
};
use windows_sys::Win32::{
    Foundation::{
        CloseHandle, GetLastError, LocalFree, ERROR_CALL_NOT_IMPLEMENTED, ERROR_CANCELLED,
        ERROR_FILE_NOT_FOUND, ERROR_PIPE_BUSY, ERROR_SERVICE_ALREADY_RUNNING,
        ERROR_SERVICE_DOES_NOT_EXIST, ERROR_SERVICE_NOT_ACTIVE, NO_ERROR,
    },
    Security::{
        Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW,
        DACL_SECURITY_INFORMATION, SECURITY_ATTRIBUTES,
    },
    Storage::FileSystem::{
        CreateDirectoryW, FILE_FLAG_OVERLAPPED, SECURITY_IDENTIFICATION, SECURITY_SQOS_PRESENT,
    },
    System::{
        Com::CoTaskMemFree,
        Pipes::{
            GetNamedPipeClientProcessId, GetNamedPipeServerProcessId, GetNamedPipeServerSessionId,
        },
        Services::{
            ChangeServiceConfig2W, ChangeServiceConfigW, CloseServiceHandle, ControlService,
            CreateServiceW, DeleteService, OpenSCManagerW, OpenServiceW, QueryServiceConfigW,
            QueryServiceStatus, RegisterServiceCtrlHandlerExW, SetServiceObjectSecurity,
            SetServiceStatus, StartServiceCtrlDispatcherW, StartServiceW, QUERY_SERVICE_CONFIGW,
            SC_ACTION, SC_ACTION_RESTART, SC_HANDLE, SC_MANAGER_ALL_ACCESS, SC_MANAGER_CONNECT,
            SERVICE_ACCEPT_SHUTDOWN, SERVICE_ACCEPT_STOP, SERVICE_ALL_ACCESS,
            SERVICE_CONFIG_DESCRIPTION, SERVICE_CONFIG_FAILURE_ACTIONS,
            SERVICE_CONTROL_INTERROGATE, SERVICE_CONTROL_SHUTDOWN, SERVICE_CONTROL_STOP,
            SERVICE_DEMAND_START, SERVICE_DESCRIPTIONW, SERVICE_ERROR_NORMAL,
            SERVICE_FAILURE_ACTIONSW, SERVICE_QUERY_CONFIG, SERVICE_QUERY_STATUS, SERVICE_RUNNING,
            SERVICE_START, SERVICE_START_PENDING, SERVICE_STATUS, SERVICE_STOPPED,
            SERVICE_STOP_PENDING, SERVICE_TABLE_ENTRYW, SERVICE_WIN32_OWN_PROCESS,
        },
        Threading::{
            GetExitCodeProcess, OpenProcess, QueryFullProcessImageNameW, WaitForSingleObject,
            PROCESS_QUERY_LIMITED_INFORMATION,
        },
    },
    UI::Shell::{
        FOLDERID_ProgramFiles, SHGetKnownFolderPath, ShellExecuteExW, SEE_MASK_NOASYNC,
        SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW,
    },
};

pub const NAME: &str = "LumenTunnel";
const DISPLAY_NAME: &str = "Lumen Tunnel";
const DESCRIPTION: &str =
    "Creates the Lumen VPN tunnel adapter and routes, so Lumen does not ask for administrator permission on every launch.";
/// Folder under Program Files and the service binary inside it.
const DIRECTORY: &str = "Lumen Tunnel";
const BINARY: &str = "lumen-tunnel-service.exe";
const PIPE: &str = r"\\.\pipe\lumen-tunnel-service";
/// "LUMN" and the hello layout; a client with another layout is another build.
const MAGIC: u32 = 0x4C55_4D4E;
const PROTOCOL: u32 = 1;
/// The service exits this long after its last session ended.
const IDLE_EXIT: Duration = Duration::from_secs(5 * 60);

const READY: u8 = 1;
const REINSTALL: u8 = 2;
const BUSY: u8 = 3;

/// SYSTEM and administrators own the pipe. Interactive users may read, write
/// and set the pipe mode, but not create instances of it.
const PIPE_SDDL: &str = "D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;0x12018b;;;IU)";
/// The access a client asks for: exactly what PIPE_SDDL grants.
const CLIENT_ACCESS: u32 = 0x0012_018B;
/// Interactive users may query and start the service; only administrators
/// change, stop or delete it.
const SERVICE_SDDL: &str =
    "D:(A;;CCLCSWRPWPDTLOCRRC;;;SY)(A;;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;BA)(A;;CCLCSWRPLOCRRC;;;IU)";
/// The service folder: administrators and SYSTEM write, users read and execute.
const DIRECTORY_SDDL: &str = "O:BAD:PAI(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;0x1200a9;;;BU)";

fn wide(value: impl AsRef<std::ffi::OsStr>) -> Vec<u16> {
    value.as_ref().encode_wide().chain(Some(0)).collect()
}

fn failure(message: &'static str) -> BackendError {
    BackendError::new("TUN_FAILED", message)
}

/// The service copy never opens the Lumen window: it is not a Lumen install.
pub fn is_service_binary() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .is_some_and(|name| name.eq_ignore_ascii_case(BINARY))
}

fn program_files() -> Option<PathBuf> {
    // The known folder comes from the machine registry. %ProgramFiles% would
    // let a user variable move the service binary somewhere the user can write.
    unsafe {
        let mut path = std::ptr::null_mut();
        if SHGetKnownFolderPath(&FOLDERID_ProgramFiles, 0, std::ptr::null_mut(), &mut path) != 0 {
            CoTaskMemFree(path as _);
            return None;
        }
        let mut length = 0;
        while *path.add(length) != 0 {
            length += 1;
        }
        let value = String::from_utf16_lossy(std::slice::from_raw_parts(path, length));
        CoTaskMemFree(path as _);
        Some(PathBuf::from(value))
    }
}

fn service_binary() -> Option<PathBuf> {
    Some(program_files()?.join(DIRECTORY).join(BINARY))
}

/// Windows paths compare without case; `\\?\` prefixes and 8.3 names differ only in form.
fn same_path(left: &Path, right: &Path) -> bool {
    let normal = |path: &Path| {
        path.canonicalize()
            .unwrap_or_else(|_| path.to_path_buf())
            .to_string_lossy()
            .trim_start_matches(r"\\?\")
            .to_lowercase()
    };
    normal(left) == normal(right)
}

/// The executable a `binPath` names: quoted, or up to the first space.
fn command_binary(command: &str) -> PathBuf {
    let command = command.trim();
    match command.strip_prefix('"') {
        Some(rest) => PathBuf::from(rest.split('"').next().unwrap_or_default()),
        None => PathBuf::from(command.split(' ').next().unwrap_or_default()),
    }
}

fn digest(path: &Path) -> Option<[u8; 32]> {
    let bytes = std::fs::read(path).ok()?;
    Some(Sha256::digest(&bytes).into())
}

fn process_image(pid: u32) -> Option<PathBuf> {
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return None;
        }
        let mut path = vec![0u16; 32768];
        let mut size = path.len() as u32;
        let read = QueryFullProcessImageNameW(process, 0, path.as_mut_ptr(), &mut size);
        CloseHandle(process);
        (read != 0).then(|| PathBuf::from(String::from_utf16_lossy(&path[..size as usize])))
    }
}

struct ServiceHandle(SC_HANDLE);
impl ServiceHandle {
    fn new(handle: SC_HANDLE) -> std::result::Result<Self, u32> {
        if handle.is_null() {
            Err(unsafe { GetLastError() })
        } else {
            Ok(Self(handle))
        }
    }
}
impl Drop for ServiceHandle {
    fn drop(&mut self) {
        unsafe {
            CloseServiceHandle(self.0);
        }
    }
}

fn with_descriptor<T>(sddl: &str, apply: impl FnOnce(*mut std::ffi::c_void) -> T) -> Option<T> {
    let sddl = wide(sddl);
    unsafe {
        let mut descriptor = std::ptr::null_mut();
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            1,
            &mut descriptor,
            std::ptr::null_mut(),
        ) == 0
        {
            return None;
        }
        let result = apply(descriptor);
        LocalFree(descriptor);
        Some(result)
    }
}

// Service side -----------------------------------------------------------------

static STATUS: AtomicUsize = AtomicUsize::new(0);
static CHECKPOINT: AtomicU32 = AtomicU32::new(0);
static STOP: OnceLock<watch::Sender<bool>> = OnceLock::new();

fn report(state: u32, exit_code: u32, wait_hint: u32) {
    let handle = STATUS.load(Ordering::SeqCst);
    if handle == 0 {
        return;
    }
    let pending = state == SERVICE_START_PENDING || state == SERVICE_STOP_PENDING;
    let status = SERVICE_STATUS {
        dwServiceType: SERVICE_WIN32_OWN_PROCESS,
        dwCurrentState: state,
        dwControlsAccepted: if state == SERVICE_RUNNING {
            SERVICE_ACCEPT_STOP | SERVICE_ACCEPT_SHUTDOWN
        } else {
            0
        },
        dwWin32ExitCode: exit_code,
        dwServiceSpecificExitCode: 0,
        dwCheckPoint: if pending {
            CHECKPOINT.fetch_add(1, Ordering::SeqCst) + 1
        } else {
            0
        },
        dwWaitHint: wait_hint,
    };
    unsafe {
        SetServiceStatus(handle as _, &status);
    }
}

unsafe extern "system" fn control(
    code: u32,
    _event: u32,
    _data: *mut std::ffi::c_void,
    _context: *mut std::ffi::c_void,
) -> u32 {
    match code {
        SERVICE_CONTROL_STOP | SERVICE_CONTROL_SHUTDOWN => {
            report(SERVICE_STOP_PENDING, NO_ERROR, 15_000);
            if let Some(stop) = STOP.get() {
                let _ = stop.send(true);
            }
            NO_ERROR
        }
        SERVICE_CONTROL_INTERROGATE => NO_ERROR,
        _ => ERROR_CALL_NOT_IMPLEMENTED,
    }
}

unsafe extern "system" fn service_main(_argc: u32, _argv: *mut windows_sys::core::PWSTR) {
    let name = wide(NAME);
    let handle = RegisterServiceCtrlHandlerExW(name.as_ptr(), Some(control), std::ptr::null());
    if handle.is_null() {
        return;
    }
    STATUS.store(handle as usize, Ordering::SeqCst);
    report(SERVICE_START_PENDING, NO_ERROR, 5_000);
    let (stop, stopped) = watch::channel(false);
    let _ = STOP.set(stop);
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build();
    let exit_code = match runtime {
        Ok(runtime) => {
            report(SERVICE_RUNNING, NO_ERROR, 0);
            runtime.block_on(serve(stopped));
            runtime.shutdown_timeout(Duration::from_secs(2));
            NO_ERROR
        }
        Err(_) => 1,
    };
    report(SERVICE_STOPPED, exit_code, 0);
}

/// `--lumen-service`: hand the process to the service control manager. Started
/// any other way, the dispatcher refuses and the process exits.
pub fn run() {
    let mut name = wide(NAME);
    let table = [
        SERVICE_TABLE_ENTRYW {
            lpServiceName: name.as_mut_ptr(),
            lpServiceProc: Some(service_main),
        },
        SERVICE_TABLE_ENTRYW {
            lpServiceName: std::ptr::null_mut(),
            lpServiceProc: None,
        },
    ];
    unsafe {
        StartServiceCtrlDispatcherW(table.as_ptr());
    }
}

fn create_pipe(first: bool) -> std::io::Result<NamedPipeServer> {
    with_descriptor(PIPE_SDDL, |descriptor| unsafe {
        let mut attributes = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor,
            bInheritHandle: 0,
        };
        ServerOptions::new()
            // A pipe someone created before the service is not ours to serve.
            .first_pipe_instance(first)
            .reject_remote_clients(true)
            .create_with_security_attributes_raw(PIPE, &mut attributes as *mut _ as _)
    })
    .unwrap_or_else(|| Err(std::io::Error::last_os_error()))
}

struct Shared {
    own: [u8; 32],
    active: AtomicUsize,
    last: std::sync::Mutex<Instant>,
    /// One tunnel session at a time: there is one LumenTun adapter.
    turn: tokio::sync::Mutex<()>,
}

async fn serve(mut stop: watch::Receiver<bool>) {
    super::helper_log::install_logger();
    super::helper::watch_shutdown(stop.clone());
    let Some(own) = std::env::current_exe().ok().and_then(|path| digest(&path)) else {
        return;
    };
    let shared = Arc::new(Shared {
        own,
        active: AtomicUsize::new(0),
        last: std::sync::Mutex::new(Instant::now()),
        turn: tokio::sync::Mutex::new(()),
    });
    let Ok(mut server) = create_pipe(true) else {
        return;
    };
    loop {
        tokio::select! {
            connected = server.connect() => {
                let mut next = None;
                for _ in 0..20 {
                    if let Ok(created) = create_pipe(false) {
                        next = Some(created);
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(250)).await;
                }
                let Some(next) = next else { break };
                let client = std::mem::replace(&mut server, next);
                if connected.is_ok() {
                    let shared = shared.clone();
                    shared.active.fetch_add(1, Ordering::SeqCst);
                    tokio::spawn(async move {
                        accept(client, &shared).await;
                        if let Ok(mut last) = shared.last.lock() {
                            *last = Instant::now();
                        }
                        shared.active.fetch_sub(1, Ordering::SeqCst);
                    });
                }
            }
            _ = stop.wait_for(|stopping| *stopping) => break,
            _ = tokio::time::sleep(Duration::from_secs(30)) => {
                let idle = shared.last.lock().is_ok_and(|last| last.elapsed() >= IDLE_EXIT);
                if shared.active.load(Ordering::SeqCst) == 0 && idle {
                    break;
                }
            }
        }
    }
    drop(server);
    // Sessions see the same stop signal and take their routes down first.
    let deadline = Instant::now() + Duration::from_secs(12);
    while shared.active.load(Ordering::SeqCst) > 0 && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

async fn accept(mut pipe: NamedPipeServer, shared: &Shared) {
    let started = Instant::now();
    let hello = tokio::time::timeout(Duration::from_secs(10), async {
        let magic = pipe.read_u32().await?;
        let protocol = pipe.read_u32().await?;
        let mut id = [0u8; 36];
        pipe.read_exact(&mut id).await?;
        Ok::<_, std::io::Error>((magic, protocol, id))
    })
    .await;
    let Ok(Ok((magic, protocol, id))) = hello else {
        return;
    };
    if magic != MAGIC {
        return;
    }
    let Some(id) = std::str::from_utf8(&id)
        .ok()
        .filter(|id| uuid::Uuid::parse_str(id).is_ok())
        .map(str::to_owned)
    else {
        return;
    };
    let mut client = 0;
    if unsafe { GetNamedPipeClientProcessId(pipe.as_raw_handle() as _, &mut client) } == 0 {
        return;
    }
    let Some(image) = process_image(client) else {
        return;
    };
    // Another build would bring other hashes and another config builder.
    if protocol != PROTOCOL || digest(&image) != Some(shared.own) {
        let _ = pipe.write_u8(REINSTALL).await;
        return;
    }
    let Ok(_turn) = tokio::time::timeout(Duration::from_secs(30), shared.turn.lock()).await else {
        let _ = pipe.write_u8(BUSY).await;
        return;
    };
    if pipe.write_u8(READY).await.is_err() {
        return;
    }
    super::helper_log::attach(&id, client);
    super::helper::set_runtime_base(image.parent().map(Path::to_path_buf));
    log::info!("Tunnel service session started");
    let result = super::helper::serve_session(&mut pipe, started).await;
    match &result {
        Ok(()) => log::info!(
            "Tunnel service session ended after {} s",
            started.elapsed().as_secs()
        ),
        Err(failure) => log::warn!(
            "Tunnel service session ended after {} s: {} ({})",
            started.elapsed().as_secs(),
            failure.code,
            failure.message
        ),
    }
    super::helper::set_runtime_base(None);
    super::helper_log::finish();
}

// Lumen side -------------------------------------------------------------------

pub enum Opened {
    /// Connected to a service built from this binary; the session can start.
    Ready(NamedPipeClient),
    /// Missing, or another build: install or update it (one UAC prompt).
    Install,
    /// Disabled, busy or failing: use a helper for this session.
    Unavailable,
}

/// Set when installing failed in this run, so later connections do not ask again.
static INSTALL_FAILED: AtomicBool = AtomicBool::new(false);

enum Installed {
    Yes,
    No,
    Unusable,
}

/// Check the registration and start a stopped service.
fn installed() -> Installed {
    let Some(expected) = service_binary() else {
        return Installed::Unusable;
    };
    unsafe {
        let Ok(manager) = ServiceHandle::new(OpenSCManagerW(
            std::ptr::null(),
            std::ptr::null(),
            SC_MANAGER_CONNECT,
        )) else {
            return Installed::Unusable;
        };
        let name = wide(NAME);
        let service = match ServiceHandle::new(OpenServiceW(
            manager.0,
            name.as_ptr(),
            SERVICE_QUERY_CONFIG | SERVICE_QUERY_STATUS | SERVICE_START,
        )) {
            Ok(service) => service,
            // Missing, or a registration without user start rights: (re)installing fixes it.
            Err(_) => return Installed::No,
        };
        let mut needed = 0;
        QueryServiceConfigW(service.0, std::ptr::null_mut(), 0, &mut needed);
        let mut buffer = vec![0u64; (needed as usize).div_ceil(8).max(1)];
        let config = buffer.as_mut_ptr() as *mut QUERY_SERVICE_CONFIGW;
        if QueryServiceConfigW(service.0, config, needed, &mut needed) == 0 {
            return Installed::Unusable;
        }
        let command = (*config).lpBinaryPathName;
        if command.is_null() {
            return Installed::No;
        }
        let mut length = 0;
        while *command.add(length) != 0 {
            length += 1;
        }
        let command = String::from_utf16_lossy(std::slice::from_raw_parts(command, length));
        if !same_path(&command_binary(&command), &expected) {
            return Installed::No;
        }
        let mut status: SERVICE_STATUS = std::mem::zeroed();
        if QueryServiceStatus(service.0, &mut status) == 0 {
            return Installed::Unusable;
        }
        if status.dwCurrentState == SERVICE_STOPPED
            && StartServiceW(service.0, 0, std::ptr::null()) == 0
            && GetLastError() != ERROR_SERVICE_ALREADY_RUNNING
        {
            // Disabled by the user or a policy: respect that, do not reinstall.
            return Installed::Unusable;
        }
        Installed::Yes
    }
}

fn open_pipe() -> std::io::Result<NamedPipeClient> {
    let file = std::fs::OpenOptions::new()
        .access_mode(CLIENT_ACCESS)
        .custom_flags(FILE_FLAG_OVERLAPPED)
        // The service must not act with Lumen's identity.
        .security_qos_flags(SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION)
        .open(PIPE)?;
    unsafe { NamedPipeClient::from_raw_handle(file.into_raw_handle()) }
}

async fn connect_pipe(wait: Duration) -> Option<NamedPipeClient> {
    let deadline = Instant::now() + wait;
    loop {
        match open_pipe() {
            Ok(pipe) => return Some(pipe),
            // Starting, or between two instances.
            Err(error)
                if matches!(
                    error.raw_os_error().map(|code| code as u32),
                    Some(ERROR_FILE_NOT_FOUND | ERROR_PIPE_BUSY)
                ) && Instant::now() < deadline =>
            {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            Err(_) => return None,
        }
    }
}

/// Connect to the service and introduce this build.
pub async fn open() -> Opened {
    if INSTALL_FAILED.load(Ordering::SeqCst) {
        return Opened::Unavailable;
    }
    let mut connected = None;
    // A service that is just stopping after its idle timeout is started again.
    for _ in 0..3 {
        match tokio::task::spawn_blocking(installed).await {
            Ok(Installed::Yes) => {}
            Ok(Installed::No) => return Opened::Install,
            _ => return Opened::Unavailable,
        }
        connected = connect_pipe(Duration::from_secs(5)).await;
        if connected.is_some() {
            break;
        }
    }
    let Some(mut pipe) = connected else {
        log::warn!("Tunnel service did not open its pipe");
        return Opened::Unavailable;
    };
    let mut session = 0;
    let mut server = 0;
    let trusted = unsafe {
        GetNamedPipeServerSessionId(pipe.as_raw_handle() as _, &mut session) != 0
            && session == 0
            && GetNamedPipeServerProcessId(pipe.as_raw_handle() as _, &mut server) != 0
    };
    if !trusted {
        log::warn!("Tunnel service pipe is not owned by a service");
        return Opened::Unavailable;
    }
    let id = uuid::Uuid::new_v4().to_string();
    let log_pipe = ServerOptions::new()
        .first_pipe_instance(true)
        .reject_remote_clients(true)
        .access_outbound(false)
        .create(super::helper_log::pipe_name(&id))
        .ok();
    let hello = async {
        pipe.write_u32(MAGIC).await?;
        pipe.write_u32(PROTOCOL).await?;
        pipe.write_all(id.as_bytes()).await?;
        pipe.read_u8().await
    };
    // The service may first finish another session's teardown.
    match tokio::time::timeout(Duration::from_secs(45), hello).await {
        Ok(Ok(READY)) => {
            if let Some(log_pipe) = log_pipe {
                super::helper_log::forward(log_pipe, server);
            }
            Opened::Ready(pipe)
        }
        Ok(Ok(REINSTALL)) => Opened::Install,
        Ok(Ok(BUSY)) => {
            log::warn!("Tunnel service is busy with another session");
            Opened::Unavailable
        }
        _ => Opened::Unavailable,
    }
}

/// Install or update the service with one UAC prompt.
pub async fn install_elevated() -> Result<()> {
    let result = tokio::task::spawn_blocking(|| unsafe {
        let executable =
            std::env::current_exe().map_err(|_| failure("Lumen executable not found"))?;
        let file = wide(&executable);
        let verb = wide("runas");
        let arguments = wide("--lumen-install-service");
        let mut info: SHELLEXECUTEINFOW = std::mem::zeroed();
        info.cbSize = std::mem::size_of_val(&info) as u32;
        info.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC;
        info.lpVerb = verb.as_ptr();
        info.lpFile = file.as_ptr();
        info.lpParameters = arguments.as_ptr();
        info.nShow = 0;
        if ShellExecuteExW(&mut info) == 0 || info.hProcess.is_null() {
            let declined = GetLastError() == ERROR_CANCELLED;
            return Err(BackendError::new(
                "PERMISSION_DENIED",
                if declined {
                    "Administrator permission was not granted for the tunnel"
                } else {
                    "Windows could not ask for administrator permission"
                },
            ));
        }
        WaitForSingleObject(info.hProcess, 120_000);
        let mut code = 1;
        let finished = GetExitCodeProcess(info.hProcess, &mut code) != 0 && code == 0;
        CloseHandle(info.hProcess);
        if finished {
            Ok(())
        } else {
            log::warn!("Tunnel service installer exited with code {code}");
            Err(failure("The tunnel service could not be installed"))
        }
    })
    .await
    .unwrap_or_else(|_| Err(failure("The tunnel service could not be installed")));
    if result
        .as_ref()
        .is_err_and(|error| error.code != "PERMISSION_DENIED")
    {
        INSTALL_FAILED.store(true, Ordering::SeqCst);
    }
    result
}

// Installer (elevated) ---------------------------------------------------------

fn stop_and_wait(service: &ServiceHandle) -> std::result::Result<(), u32> {
    unsafe {
        let mut status: SERVICE_STATUS = std::mem::zeroed();
        if ControlService(service.0, SERVICE_CONTROL_STOP, &mut status) == 0 {
            let error = GetLastError();
            if error != ERROR_SERVICE_NOT_ACTIVE {
                return Err(error);
            }
        }
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if QueryServiceStatus(service.0, &mut status) == 0 {
                return Err(GetLastError());
            }
            if status.dwCurrentState == SERVICE_STOPPED {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(1);
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    }
}

/// A fresh folder with a protected ACL, then the binary. The process of a
/// stopped service can hold its image for a moment, so removal retries.
fn replace_binary(source: &Path, directory: &Path) -> std::result::Result<PathBuf, u32> {
    let deadline = Instant::now() + Duration::from_secs(15);
    while directory.exists() {
        if std::fs::remove_dir_all(directory).is_ok() {
            break;
        }
        if Instant::now() >= deadline {
            return Err(32);
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    let created = with_descriptor(DIRECTORY_SDDL, |descriptor| unsafe {
        let security = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor,
            bInheritHandle: 0,
        };
        CreateDirectoryW(wide(directory).as_ptr(), &security) != 0
    });
    if created != Some(true) {
        return Err(unsafe { GetLastError() });
    }
    let binary = directory.join(BINARY);
    std::fs::copy(source, &binary).map_err(|error| error.raw_os_error().unwrap_or(1) as u32)?;
    Ok(binary)
}

fn configure(service: &ServiceHandle) {
    unsafe {
        let mut description = wide(DESCRIPTION);
        let description = SERVICE_DESCRIPTIONW {
            lpDescription: description.as_mut_ptr(),
        };
        ChangeServiceConfig2W(
            service.0,
            SERVICE_CONFIG_DESCRIPTION,
            &description as *const _ as _,
        );
        let mut actions = [
            SC_ACTION {
                Type: SC_ACTION_RESTART,
                Delay: 2_000,
            },
            SC_ACTION {
                Type: SC_ACTION_RESTART,
                Delay: 5_000,
            },
        ];
        let failure_actions = SERVICE_FAILURE_ACTIONSW {
            dwResetPeriod: 24 * 60 * 60,
            lpRebootMsg: std::ptr::null_mut(),
            lpCommand: std::ptr::null_mut(),
            cActions: actions.len() as u32,
            lpsaActions: actions.as_mut_ptr(),
        };
        ChangeServiceConfig2W(
            service.0,
            SERVICE_CONFIG_FAILURE_ACTIONS,
            &failure_actions as *const _ as _,
        );
    }
}

fn install_inner() -> std::result::Result<(), u32> {
    let source = std::env::current_exe().map_err(|_| 2u32)?;
    let directory = program_files().ok_or(3u32)?.join(DIRECTORY);
    unsafe {
        let manager = ServiceHandle::new(OpenSCManagerW(
            std::ptr::null(),
            std::ptr::null(),
            SC_MANAGER_ALL_ACCESS,
        ))?;
        let name = wide(NAME);
        let existing =
            ServiceHandle::new(OpenServiceW(manager.0, name.as_ptr(), SERVICE_ALL_ACCESS)).ok();
        if let Some(service) = &existing {
            stop_and_wait(service)?;
        }
        let binary = replace_binary(&source, &directory)?;
        let command = wide(format!("\"{}\" --lumen-service", binary.display()));
        let display = wide(DISPLAY_NAME);
        let account = wide("LocalSystem");
        let password = wide("");
        let service = match existing {
            Some(service) => {
                if ChangeServiceConfigW(
                    service.0,
                    SERVICE_WIN32_OWN_PROCESS,
                    SERVICE_DEMAND_START,
                    SERVICE_ERROR_NORMAL,
                    command.as_ptr(),
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    account.as_ptr(),
                    password.as_ptr(),
                    display.as_ptr(),
                ) == 0
                {
                    return Err(GetLastError());
                }
                service
            }
            None => ServiceHandle::new(CreateServiceW(
                manager.0,
                name.as_ptr(),
                display.as_ptr(),
                SERVICE_ALL_ACCESS,
                SERVICE_WIN32_OWN_PROCESS,
                SERVICE_DEMAND_START,
                SERVICE_ERROR_NORMAL,
                command.as_ptr(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
            ))?,
        };
        configure(&service);
        let secured = with_descriptor(SERVICE_SDDL, |descriptor| {
            SetServiceObjectSecurity(service.0, DACL_SECURITY_INFORMATION, descriptor) != 0
        });
        if secured != Some(true) {
            return Err(GetLastError());
        }
        if StartServiceW(service.0, 0, std::ptr::null()) == 0
            && GetLastError() != ERROR_SERVICE_ALREADY_RUNNING
        {
            return Err(GetLastError());
        }
        Ok(())
    }
}

/// `--lumen-install-service`, run elevated: install or update, then start.
pub fn install() -> i32 {
    match install_inner() {
        Ok(()) => 0,
        Err(code) => code.max(1) as i32,
    }
}

/// `--lumen-uninstall-service`, run elevated by the uninstaller.
pub fn uninstall() -> i32 {
    let result = (|| unsafe {
        let manager = ServiceHandle::new(OpenSCManagerW(
            std::ptr::null(),
            std::ptr::null(),
            SC_MANAGER_ALL_ACCESS,
        ))?;
        let name = wide(NAME);
        match ServiceHandle::new(OpenServiceW(manager.0, name.as_ptr(), SERVICE_ALL_ACCESS)) {
            Ok(service) => {
                let _ = stop_and_wait(&service);
                if DeleteService(service.0) == 0 {
                    return Err(GetLastError());
                }
            }
            Err(ERROR_SERVICE_DOES_NOT_EXIST) => {}
            Err(error) => return Err(error),
        }
        if let Some(directory) = program_files().map(|path| path.join(DIRECTORY)) {
            let deadline = Instant::now() + Duration::from_secs(15);
            while directory.exists()
                && std::fs::remove_dir_all(&directory).is_err()
                && Instant::now() < deadline
            {
                std::thread::sleep(Duration::from_millis(250));
            }
        }
        Ok::<(), u32>(())
    })();
    match result {
        Ok(()) => 0,
        Err(code) => code.max(1) as i32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_command_names_the_quoted_binary() {
        assert_eq!(
            command_binary(
                r#""C:\Program Files\Lumen Tunnel\lumen-tunnel-service.exe" --lumen-service"#
            ),
            PathBuf::from(r"C:\Program Files\Lumen Tunnel\lumen-tunnel-service.exe")
        );
        assert_eq!(
            command_binary(r"C:\Windows\service.exe --flag"),
            PathBuf::from(r"C:\Windows\service.exe")
        );
    }

    #[test]
    fn paths_compare_without_case_or_verbatim_prefix() {
        assert!(same_path(
            Path::new(r"C:\Program Files\Lumen Tunnel\X.exe"),
            Path::new(r"\\?\c:\program files\lumen tunnel\x.exe")
        ));
        assert!(!same_path(
            Path::new(r"C:\Program Files\Lumen Tunnel\x.exe"),
            Path::new(r"C:\Users\x\AppData\Local\Lumen\x.exe")
        ));
    }

    #[test]
    fn service_binary_lives_in_program_files() {
        let binary = service_binary().unwrap();
        let program_files = program_files().unwrap();
        assert!(binary.starts_with(&program_files));
        assert!(binary.ends_with(Path::new(DIRECTORY).join(BINARY)));
    }

    #[test]
    fn descriptors_parse() {
        for sddl in [PIPE_SDDL, SERVICE_SDDL, DIRECTORY_SDDL] {
            assert_eq!(with_descriptor(sddl, |_| ()), Some(()), "{sddl}");
        }
    }

    #[test]
    fn client_access_is_what_the_pipe_grants() {
        assert!(PIPE_SDDL.ends_with(&format!("(A;;{:#x};;;IU)", CLIENT_ACCESS)));
        // FILE_APPEND_DATA is FILE_CREATE_PIPE_INSTANCE on a pipe: never granted.
        assert_eq!(CLIENT_ACCESS & 0x4, 0);
    }
}
