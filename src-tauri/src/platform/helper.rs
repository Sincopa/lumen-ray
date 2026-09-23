//! Single-session elevated worker. Neither paths nor executable arguments are accepted over IPC.
use crate::{
    domain::errors::{BackendError, Result},
    storage::repository::Repository,
};
use std::{os::windows::io::AsRawHandle, time::Duration};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::windows::named_pipe::{ClientOptions, NamedPipeServer, ServerOptions},
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE},
    System::{
        Pipes::{GetNamedPipeClientProcessId, GetNamedPipeServerProcessId},
        Threading::{
            GetProcessId, OpenProcess, QueryFullProcessImageNameW, WaitForSingleObject,
            PROCESS_QUERY_LIMITED_INFORMATION,
        },
    },
    UI::Shell::{ShellExecuteExW, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW},
};
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StartTunnel {
    pub core_version: String,
    pub repository: Repository,
    pub selected: String,
    pub proxy_port: u16,
    pub metrics_port: u16,
}
pub struct Helper {
    pipe: NamedPipeServer,
    process: ProcessHandle,
}
struct ProcessHandle(HANDLE);
unsafe impl Send for ProcessHandle {}
impl Drop for ProcessHandle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
impl Helper {
    pub async fn stop(&mut self) -> Result<()> {
        self.pipe.write_u8(0).await.map_err(|_| error())?;
        tokio::time::timeout(Duration::from_secs(10), self.pipe.read_u8())
            .await
            .map_err(|_| error())?
            .map_err(|_| error())?;
        Ok(())
    }
    pub fn exited(&self) -> bool {
        unsafe { WaitForSingleObject(self.process.0, 0) == 0 }
    }
}
fn error() -> BackendError {
    BackendError::new(
        "TUN_FAILED",
        "The tunnel helper could not complete the operation",
    )
}
fn wide(input: &std::ffi::OsStr) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    input.encode_wide().chain(Some(0)).collect()
}
fn launch(id: String) -> Result<ProcessHandle> {
    let executable = std::env::current_exe().map_err(|_| error())?;
    let file = wide(executable.as_os_str());
    let verb = wide(std::ffi::OsStr::new("runas"));
    let arguments = wide(std::ffi::OsStr::new(&format!(
        "--lumen-tunnel {id} {}",
        std::process::id()
    )));
    unsafe {
        let mut info: SHELLEXECUTEINFOW = std::mem::zeroed();
        info.cbSize = std::mem::size_of_val(&info) as u32;
        info.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC;
        info.lpVerb = verb.as_ptr();
        info.lpFile = file.as_ptr();
        info.lpParameters = arguments.as_ptr();
        info.nShow = 0;
        if ShellExecuteExW(&mut info) == 0 || info.hProcess.is_null() {
            return Err(BackendError::new(
                "PERMISSION_DENIED",
                "Administrator permission was not granted for the tunnel",
            ));
        }
        Ok(ProcessHandle(info.hProcess))
    }
}
pub async fn start(request: StartTunnel) -> Result<Helper> {
    let id = uuid::Uuid::new_v4().to_string();
    let mut pipe = ServerOptions::new()
        .first_pipe_instance(true)
        .reject_remote_clients(true)
        .create(format!(r"\\.\pipe\lumen-{id}"))
        .map_err(|_| error())?;
    let process = tokio::task::spawn_blocking(move || launch(id))
        .await
        .map_err(|_| error())??;
    tokio::time::timeout(Duration::from_secs(30), pipe.connect())
        .await
        .map_err(|_| error())?
        .map_err(|_| error())?;
    let mut pid = 0;
    unsafe {
        if GetNamedPipeClientProcessId(pipe.as_raw_handle() as _, &mut pid) == 0
            || pid != GetProcessId(process.0)
        {
            return Err(error());
        }
    }
    write_frame(
        &mut pipe,
        &serde_json::to_vec(&request).map_err(|_| error())?,
    )
    .await?;
    let ready = tokio::time::timeout(Duration::from_secs(120), pipe.read_u8())
        .await
        .map_err(|_| error())?
        .map_err(|_| error())?;
    if ready != 1 {
        return Err(error());
    }
    Ok(Helper { pipe, process })
}
async fn write_frame<W: AsyncWrite + Unpin>(writer: &mut W, bytes: &[u8]) -> Result<()> {
    if bytes.len() > 8 * 1024 * 1024 {
        return Err(error());
    }
    writer
        .write_u32(bytes.len() as u32)
        .await
        .map_err(|_| error())?;
    writer.write_all(bytes).await.map_err(|_| error())?;
    Ok(())
}
async fn read_frame<R: AsyncRead + Unpin>(reader: &mut R) -> Result<Vec<u8>> {
    let size = reader.read_u32().await.map_err(|_| error())? as usize;
    if size > 8 * 1024 * 1024 {
        return Err(error());
    }
    let mut bytes = vec![0; size];
    reader.read_exact(&mut bytes).await.map_err(|_| error())?;
    Ok(bytes)
}
fn verify_parent(pid: u32) -> Result<ProcessHandle> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return Err(error());
        }
        let handle = ProcessHandle(handle);
        let mut path = vec![0u16; 32768];
        let mut size = path.len() as u32;
        if QueryFullProcessImageNameW(handle.0, 0, path.as_mut_ptr(), &mut size) == 0 {
            return Err(error());
        }
        let parent = std::path::PathBuf::from(String::from_utf16_lossy(&path[..size as usize]));
        if parent != std::env::current_exe().map_err(|_| error())? {
            return Err(error());
        }
        Ok(handle)
    }
}
fn resolve_resources() -> Result<std::path::PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            candidates.push(parent.join("resources/core"));
            candidates.push(parent.join("../src-tauri/resources/core"));
        }
    }
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        candidates.push(
            std::path::PathBuf::from(local_app_data)
                .join("app.lumen.desktop")
                .join("core")
                .join(crate::xray::core::VERSION),
        );
    }
    if cfg!(debug_assertions) {
        candidates
            .push(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/core"));
    }
    for candidate in candidates {
        if candidate.join("xray.exe").is_file() {
            return Ok(candidate);
        }
    }
    Err(error())
}
pub async fn run(id: &str, parent: u32) -> Result<()> {
    uuid::Uuid::parse_str(id).map_err(|_| error())?;
    let _parent = verify_parent(parent)?;
    let mut pipe = ClientOptions::new()
        .open(format!(r"\\.\pipe\lumen-{id}"))
        .map_err(|_| error())?;
    let mut pid = 0;
    unsafe {
        if GetNamedPipeServerProcessId(pipe.as_raw_handle() as _, &mut pid) == 0 || pid != parent {
            return Err(error());
        }
    }
    let bytes = tokio::time::timeout(Duration::from_secs(15), read_frame(&mut pipe))
        .await
        .map_err(|_| error())??;
    let request: StartTunnel = serde_json::from_slice(&bytes).map_err(|_| error())?;
    let config = crate::xray::config::build(
        &request.repository,
        &request.selected,
        request.proxy_port,
        request.metrics_port,
        true,
    )?;
    let bytes = serde_json::to_vec(&config).map_err(|_| error())?;
    let resources = resolve_resources()?;
    let staging = super::staging::Directory::create()?;
    let data = tempfile::tempdir_in(staging.path()).map_err(|_| error())?;
    let core = if request.core_version == crate::xray::core::VERSION {
        crate::xray::core::prepare(&resources, data.path())?
    } else {
        crate::xray::updater::prepare(&request.core_version, &data.path().join("core")).await?
    };
    if let Some(parent) = core.parent() {
        std::env::set_var("XRAY_LOCATION_ASSET", parent);
    }
    crate::services::vpn_service::validate(&core, &bytes).await?;
    let mut child = super::process::CoreProcess::spawn(&core, &bytes).await?;
    // Closing the authenticated pipe (including parent crash) tears down the child and Wintun.
    let mut ready = false;
    for _ in 0..200 {
        if child.try_wait().map_err(|_| error())?.is_some() {
            return Err(error());
        }
        if super::network::tunnel_ready() {
            ready = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    if !ready {
        return Err(error());
    }
    pipe.write_u8(1).await.map_err(|_| error())?;
    tokio::select! {_=pipe.read_u8()=>{},_=child.wait()=>{}}
    if child.try_wait().map_err(|_| error())?.is_none() {
        child.kill().await.map_err(|_| error())?;
    }
    let _ = pipe.write_u8(0).await;
    Ok(())
}
