//! Single-session elevated worker. Neither paths nor executable arguments are accepted over IPC.
use crate::{
    domain::errors::{BackendError, Result},
    storage::repository::Repository,
};
use std::{os::windows::io::AsRawHandle, time::{Duration, Instant}};
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
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct StartRequest {
    tunnel: StartTunnel,
    geo: crate::xray::geo::GeoDatabaseMetadata,
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
pub async fn start(request: StartTunnel, data: &std::path::Path) -> Result<Helper> {
    let started_at = std::time::Instant::now();
    let (geo, geoip, geosite) = crate::xray::geo::snapshot(data)?;
    log::info!("VPN timing: TUN assets verified {} ms", started_at.elapsed().as_millis());
    let request = StartRequest { tunnel: request, geo };
    let id = uuid::Uuid::new_v4().to_string();
    let mut pipe = ServerOptions::new()
        .first_pipe_instance(true)
        .reject_remote_clients(true)
        .create(format!(r"\\.\pipe\lumen-{id}"))
        .map_err(|_| error())?;
    let process = tokio::task::spawn_blocking(move || launch(id))
        .await
        .map_err(|_| error())??;
    log::info!("VPN timing: TUN helper launched {} ms", started_at.elapsed().as_millis());
    tokio::time::timeout(Duration::from_secs(30), pipe.connect())
        .await
        .map_err(|_| error())?
        .map_err(|_| error())?;
    log::info!("VPN timing: TUN pipe connected {} ms", started_at.elapsed().as_millis());
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
    write_asset(&mut pipe, &geoip, crate::xray::geo::MAX_GEOIP_SIZE).await?;
    write_asset(&mut pipe, &geosite, crate::xray::geo::MAX_GEOSITE_SIZE).await?;
    log::info!("VPN timing: TUN assets transferred {} ms", started_at.elapsed().as_millis());
    let ready = tokio::time::timeout(Duration::from_secs(120), pipe.read_u8())
        .await
        .map_err(|_| error())?
        .map_err(|_| error())?;
    if ready != 1 {
        return Err(error());
    }
    let mut stages = [0u32; 7];
    tokio::time::timeout(Duration::from_secs(5), async {
        for stage in &mut stages {
            *stage = pipe.read_u32().await.map_err(|_| error())?;
        }
        Ok::<(), BackendError>(())
    })
    .await
    .map_err(|_| error())??;
    log::info!(
        "VPN timing: TUN worker request={} assets={} config={} core={} geo={} xray={} routes={} ms",
        stages[0], stages[1], stages[2], stages[3], stages[4], stages[5], stages[6]
    );
    log::info!("VPN timing: TUN helper acknowledged {} ms", started_at.elapsed().as_millis());
    Ok(Helper { pipe, process })
}
async fn write_asset<W: AsyncWrite + Unpin>(writer: &mut W, bytes: &[u8], limit: usize) -> Result<()> {
    if bytes.len() > limit { return Err(error()); }
    writer.write_u32(bytes.len() as u32).await.map_err(|_| error())?;
    writer.write_all(bytes).await.map_err(|_| error())
}
async fn read_asset<R: AsyncRead + Unpin>(reader: &mut R, limit: usize) -> Result<Vec<u8>> {
    let size = reader.read_u32().await.map_err(|_| error())? as usize;
    if size < 1024 || size > limit { return Err(error()); }
    let mut bytes = vec![0; size];
    reader.read_exact(&mut bytes).await.map_err(|_| error())?;
    Ok(bytes)
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
fn resolve_resources() -> Result<crate::runtime::RuntimePaths> {
    crate::runtime::resolve()
}
pub async fn run(id: &str, parent: u32) -> Result<()> {
    let worker_started = Instant::now();
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
    let envelope: StartRequest = serde_json::from_slice(&bytes).map_err(|_| error())?;
    let request_ready = worker_started.elapsed().as_millis() as u32;
    let (geoip, geosite) = tokio::time::timeout(Duration::from_secs(30), async {
        let ip = read_asset(&mut pipe, crate::xray::geo::MAX_GEOIP_SIZE).await?;
        let site = read_asset(&mut pipe, crate::xray::geo::MAX_GEOSITE_SIZE).await?;
        Ok::<_, BackendError>((ip, site))
    }).await.map_err(|_| error())??;
    let assets_ready = worker_started.elapsed().as_millis() as u32;
    let request = envelope.tunnel;
    let config = crate::xray::config::build(
        &request.repository,
        &request.selected,
        request.proxy_port,
        request.metrics_port,
        true,
    )?;
    let bytes = serde_json::to_vec(&config).map_err(|_| error())?;
    let config_ready = worker_started.elapsed().as_millis() as u32;
    let runtime = resolve_resources()?;
    let staging = super::staging::Directory::create()?;
    let data = tempfile::tempdir_in(staging.path()).map_err(|_| error())?;
    // The authenticated pipe supplies the Geo snapshot; avoid initializing bundled Geo here.
    let bundled_core = if request.core_version == crate::xray::core::VERSION {
        crate::xray::core::prepare_for_tunnel(&runtime, data.path())?
    } else {
        // Updated cores are still validated against a complete bundled fallback.
        crate::xray::core::prepare(&runtime, data.path())?
    };
    let core = if request.core_version == crate::xray::core::VERSION {
        bundled_core
    } else {
        crate::xray::updater::prepare(&request.core_version, &data.path().join("core")).await?
    };
    let core_ready = worker_started.elapsed().as_millis() as u32;
    crate::xray::geo::install_trusted_snapshot(data.path(), envelope.geo, &geoip, &geosite)?;
    let geo_ready = worker_started.elapsed().as_millis() as u32;
    let mut child = super::process::CoreProcess::spawn(&core, &bytes).await?;
    let xray_spawned = worker_started.elapsed().as_millis() as u32;
    // Closing the authenticated pipe (including parent crash) tears down the child and Wintun.
    let mut ready = false;
    for _ in 0..400 {
        if child.try_wait().map_err(|_| error())?.is_some() {
            return Err(error());
        }
        if super::network::tunnel_ready() {
            ready = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    if !ready {
        return Err(error());
    }
    let routes_ready = worker_started.elapsed().as_millis() as u32;
    pipe.write_u8(1).await.map_err(|_| error())?;
    for stage in [request_ready, assets_ready, config_ready, core_ready, geo_ready, xray_spawned, routes_ready] {
        pipe.write_u32(stage).await.map_err(|_| error())?;
    }
    tokio::select! {_=pipe.read_u8()=>{},_=child.wait()=>{}}
    if child.try_wait().map_err(|_| error())?.is_none() {
        child.kill().await.map_err(|_| error())?;
    }
    let _ = pipe.write_u8(0).await;
    Ok(())
}
