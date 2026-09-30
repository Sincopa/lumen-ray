//! Single-session elevated worker. Neither paths nor executable arguments are accepted over IPC.
use crate::{
    domain::errors::{BackendError, Result},
    storage::repository::Repository,
};
use std::{
    os::windows::io::AsRawHandle,
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::windows::named_pipe::{ClientOptions, NamedPipeClient, NamedPipeServer, ServerOptions},
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
#[derive(Clone, serde::Serialize, serde::Deserialize)]
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
    standby: bool,
}
pub struct Helper {
    pipe: NamedPipeServer,
    process: ProcessHandle,
    standby: bool,
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
    pub fn is_standby(&self) -> bool {
        self.standby
    }
    pub async fn activate(&mut self) -> Result<()> {
        if !self.standby {
            return Err(error());
        }
        self.pipe.write_u8(1).await.map_err(|_| error())?;
        let reply = tokio::time::timeout(Duration::from_secs(5), self.pipe.read_u8())
            .await
            .map_err(|_| error())?
            .map_err(|_| error())?;
        match reply {
            1 => Ok(()),
            2 => Err(BackendError::new(
                "TUN_ADAPTER_TIMEOUT",
                "Windows did not make the tunnel adapter ready",
            )),
            4 => {
                if let Ok(Ok(bytes)) =
                    tokio::time::timeout(Duration::from_secs(1), read_frame(&mut self.pipe)).await
                {
                    log_route_reason(&bytes);
                }
                Err(BackendError::new(
                    "ROUTING_CONFLICT",
                    "Windows selected a route outside Lumen. Retry or check competing routes",
                ))
            }
            _ => Err(error()),
        }
    }
    pub async fn deactivate(&mut self) -> Result<()> {
        if !self.standby {
            return Err(error());
        }
        self.pipe.write_u8(2).await.map_err(|_| error())?;
        let reply = tokio::time::timeout(Duration::from_secs(5), self.pipe.read_u8())
            .await
            .map_err(|_| error())?
            .map_err(|_| error())?;
        if reply == 1 {
            Ok(())
        } else {
            Err(error())
        }
    }
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

fn log_route_reason(bytes: &[u8]) {
    if let Ok(message) = std::str::from_utf8(bytes) {
        for reason in [
            "LumenTun unavailable",
            "cloudflare lookup",
            "cloudflare selected another interface",
            "google lookup",
            "google selected another interface",
        ] {
            if message == format!("Windows selected a route outside Lumen: {reason}") {
                log::warn!("Lumen tunnel route verification failed: {reason}");
                break;
            }
        }
    }
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

#[allow(dead_code)]
pub async fn start(request: StartTunnel, data: &std::path::Path) -> Result<Helper> {
    start_internal(request, data, false).await
}

pub async fn start_standby(request: StartTunnel, data: &std::path::Path) -> Result<Helper> {
    start_internal(request, data, true).await
}
async fn start_internal(
    request: StartTunnel,
    data: &std::path::Path,
    standby: bool,
) -> Result<Helper> {
    let started_at = std::time::Instant::now();
    let (geo, geoip, geosite) = crate::xray::geo::snapshot(data)?;
    log::info!(
        "VPN timing: TUN assets verified {} ms",
        started_at.elapsed().as_millis()
    );
    let request = StartRequest {
        tunnel: request,
        geo,
        standby,
    };
    let id = uuid::Uuid::new_v4().to_string();
    let mut pipe = ServerOptions::new()
        .first_pipe_instance(true)
        .reject_remote_clients(true)
        .create(format!(r"\\.\pipe\lumen-{id}"))
        .map_err(|_| error())?;
    let process = tokio::task::spawn_blocking(move || launch(id))
        .await
        .map_err(|_| error())??;
    log::info!(
        "VPN timing: TUN helper launched {} ms",
        started_at.elapsed().as_millis()
    );
    tokio::time::timeout(Duration::from_secs(30), pipe.connect())
        .await
        .map_err(|_| error())?
        .map_err(|_| error())?;
    log::info!(
        "VPN timing: TUN pipe connected {} ms",
        started_at.elapsed().as_millis()
    );
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
    log::info!(
        "VPN timing: TUN assets transferred {} ms",
        started_at.elapsed().as_millis()
    );
    let ready = tokio::time::timeout(Duration::from_secs(120), pipe.read_u8())
        .await
        .map_err(|_| error())?
        .map_err(|_| error())?;
    if ready != 1 {
        let core_failure = if ready == 3 || ready == 4 || ready == 5 {
            tokio::time::timeout(Duration::from_secs(1), read_frame(&mut pipe))
                .await
                .ok()
                .and_then(std::result::Result::ok)
                .and_then(|bytes| String::from_utf8(bytes).ok())
                .filter(|value| {
                    if ready == 4 {
                        log_route_reason(value.as_bytes());
                    }
                    [
                        "Windows tunnel route setup failed",
                        "Windows tunnel address setup failed",
                        "Windows tunnel DNS setup failed",
                        "Windows tunnel interface is not ready",
                        "Wintun adapter startup failed",
                        "startup failure",
                        "core error (details withheld)",
                        "core warning (details withheld)",
                        "Xray exited before the tunnel was ready",
                        "Tunnel adapter is unavailable",
                        "Tunnel interface is not ready",
                        "Could not lower standby tunnel priority",
                        "Prepared tunnel is not isolated from normal networking",
                    ]
                    .contains(&value.as_str())
                })
        } else {
            None
        };
        return Err(match ready {
            2 => BackendError::new(
                "TUN_ADAPTER_TIMEOUT",
                "Windows did not make the tunnel adapter ready",
            ),
            3 => BackendError::new(
                "CORE_START_FAILED",
                core_failure.unwrap_or_else(|| "Xray exited before the tunnel was ready".into()),
            ),
            4 => BackendError::new(
                "ROUTING_CONFLICT",
                "Windows selected a route outside Lumen. Retry or check competing routes",
            ),
            5 => BackendError::new(
                "TUN_FAILED",
                core_failure.unwrap_or_else(|| error().message),
            ),
            _ => error(),
        });
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
        "engine=xray VPN timing: TUN worker request={} assets={} config={} core={} geo={} xray={} routes={} ms",
        stages[0], stages[1], stages[2], stages[3], stages[4], stages[5], stages[6]
    );
    log::info!(
        "VPN timing: TUN helper acknowledged {} ms",
        started_at.elapsed().as_millis()
    );
    Ok(Helper {
        pipe,
        process,
        standby,
    })
}
async fn write_asset<W: AsyncWrite + Unpin>(
    writer: &mut W,
    bytes: &[u8],
    limit: usize,
) -> Result<()> {
    if bytes.len() > limit {
        return Err(error());
    }
    writer
        .write_u32(bytes.len() as u32)
        .await
        .map_err(|_| error())?;
    writer.write_all(bytes).await.map_err(|_| error())
}
async fn read_asset<R: AsyncRead + Unpin>(reader: &mut R, limit: usize) -> Result<Vec<u8>> {
    let size = reader.read_u32().await.map_err(|_| error())? as usize;
    if size < 1024 || size > limit {
        return Err(error());
    }
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
    let result = run_session(&mut pipe, envelope, worker_started).await;
    if let Err(failure) = &result {
        let status = match failure.code {
            "TUN_ADAPTER_TIMEOUT" => 2,
            "CORE_START_FAILED" => 3,
            "ROUTING_CONFLICT" => 4,
            "TUN_FAILED" => 5,
            _ => 0,
        };
        let _ = pipe.write_u8(status).await;
        if status == 3 || status == 4 || status == 5 {
            let _ = write_frame(&mut pipe, failure.message.as_bytes()).await;
        }
    }
    result
}

async fn run_session(
    mut pipe: &mut NamedPipeClient,
    envelope: StartRequest,
    worker_started: Instant,
) -> Result<()> {
    let standby = envelope.standby;
    let request_ready = worker_started.elapsed().as_millis() as u32;
    let (geoip, geosite) = tokio::time::timeout(Duration::from_secs(30), async {
        let ip = read_asset(&mut pipe, crate::xray::geo::MAX_GEOIP_SIZE).await?;
        let site = read_asset(&mut pipe, crate::xray::geo::MAX_GEOSITE_SIZE).await?;
        Ok::<_, BackendError>((ip, site))
    })
    .await
    .map_err(|_| error())??;
    let assets_ready = worker_started.elapsed().as_millis() as u32;
    let request = envelope.tunnel;
    // The existing worker owns Xray only. Never interpret a native-engine request as Xray.
    if request.repository.settings.core_engine != crate::core::CoreEngine::Xray {
        return Err(BackendError::new(
            "CORE_ENGINE_UNSUPPORTED",
            "This helper session requires Xray Core",
        ));
    }
    let config = if standby {
        crate::xray::config::build_standby(
            &request.repository,
            &request.selected,
            request.proxy_port,
            request.metrics_port,
        )?
    } else {
        crate::xray::config::build(
            &request.repository,
            &request.selected,
            request.proxy_port,
            request.metrics_port,
            true,
        )?
    };
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
    let mut routes = None;
    for _ in 0..if standby { 800 } else { 400 } {
        if child.try_wait().map_err(|_| error())?.is_some() {
            tokio::time::sleep(Duration::from_millis(20)).await;
            return Err(BackendError::new(
                "CORE_START_FAILED",
                child
                    .diagnostic_category()
                    .unwrap_or("Xray exited before the tunnel was ready"),
            ));
        }
        let network_ready = if standby {
            // Xray configures both IP families before opening the SOCKS listener.
            // Do not race its interface setup by changing metrics as soon as the
            // IPv4 adapter appears: Xray can still overwrite them afterwards.
            if super::network::tunnel_adapter_ready()
                && tokio::net::TcpStream::connect((
                    std::net::Ipv4Addr::LOCALHOST,
                    request.proxy_port,
                ))
                .await
                .is_ok()
            {
                if routes.is_none() {
                    routes = Some(super::network::TunnelRoutes::new()?);
                }
                true
            } else {
                false
            }
        } else {
            super::network::tunnel_ready() && super::network::verify_tunnel_routes().is_ok()
        };
        if network_ready {
            ready = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    if !ready {
        if !standby && super::network::tunnel_ready() {
            super::network::verify_tunnel_routes()?;
        }
        return Err(BackendError::new(
            "TUN_ADAPTER_TIMEOUT",
            "Windows did not make the tunnel adapter ready",
        ));
    }
    if standby && !super::network::standby_isolated() {
        return Err(BackendError::new(
            "TUN_FAILED",
            "Prepared tunnel is not isolated from normal networking",
        ));
    }
    let routes_ready = worker_started.elapsed().as_millis() as u32;
    pipe.write_u8(1).await.map_err(|_| error())?;
    for stage in [
        request_ready,
        assets_ready,
        config_ready,
        core_ready,
        geo_ready,
        xray_spawned,
        routes_ready,
    ] {
        pipe.write_u32(stage).await.map_err(|_| error())?;
    }
    if standby {
        let routes = routes.as_mut().ok_or_else(error)?;
        loop {
            tokio::select! {
                command = pipe.read_u8() => match command {
                    Ok(1) => {
                        let mut result = routes.activate();
                        for _ in 0..8 {
                            if !matches!(&result, Err(error) if error.code == "TUN_FAILED") { break; }
                            tokio::time::sleep(Duration::from_millis(25)).await;
                            result = routes.activate();
                        }
                        let (code, detail) = match result {
                            Ok(()) => (1, None),
                            Err(e) if e.code == "ROUTING_CONFLICT" => (4, Some(e.message)),
                            Err(e) if e.code == "TUN_FAILED" => (2, None),
                            Err(_) => (0, None),
                        };
                        pipe.write_u8(code).await.map_err(|_| error())?;
                        if let Some(detail) = detail {
                            write_frame(&mut pipe, detail.as_bytes()).await?;
                        }
                    }
                    Ok(2) => {
                        pipe.write_u8(if routes.deactivate().is_ok() && super::network::standby_isolated() { 1 } else { 0 }).await.map_err(|_| error())?;
                    }
                    _ => break,
                },
                _ = child.wait() => break,
            }
        }
        let _ = routes.deactivate();
    } else {
        tokio::select! {_=pipe.read_u8()=>{},_=child.wait()=>{}}
    }
    if child.try_wait().map_err(|_| error())?.is_none() {
        child.kill().await.map_err(|_| error())?;
    }
    let _ = pipe.write_u8(0).await;
    Ok(())
}
