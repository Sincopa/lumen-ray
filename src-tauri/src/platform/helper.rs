//! Single-session elevated worker. Neither paths nor executable arguments are accepted over IPC.
//!
//! The session runs either in the Lumen Tunnel service (`super::service`, no
//! prompt) or, when the service is unavailable, in a one-off helper process
//! started through UAC.
use crate::{
    domain::errors::{BackendError, Result},
    storage::repository::Repository,
};
use std::{
    os::windows::io::AsRawHandle,
    pin::Pin,
    task::{Context, Poll},
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf},
    net::windows::named_pipe::{ClientOptions, NamedPipeClient, NamedPipeServer, ServerOptions},
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE},
    System::{
        Pipes::{GetNamedPipeClientProcessId, GetNamedPipeServerProcessId, PeekNamedPipe},
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
    pipe: Link,
    standby: bool,
}
/// The authenticated pipe to the elevated side of a session.
enum Link {
    /// A helper process started through UAC; Lumen owns the pipe.
    Process(NamedPipeServer, ProcessHandle),
    /// The Lumen Tunnel service; the service owns the pipe.
    Service(NamedPipeClient),
}
impl Link {
    fn exited(&self) -> bool {
        match self {
            Link::Process(_, process) => unsafe { WaitForSingleObject(process.0, 0) == 0 },
            // A session the service ended (or lost with the service) breaks the pipe.
            Link::Service(pipe) => unsafe {
                let mut available = 0;
                PeekNamedPipe(
                    pipe.as_raw_handle() as _,
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null_mut(),
                    &mut available,
                    std::ptr::null_mut(),
                ) == 0
            },
        }
    }
}
impl AsyncRead for Link {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            Link::Process(pipe, _) => Pin::new(pipe).poll_read(cx, buf),
            Link::Service(pipe) => Pin::new(pipe).poll_read(cx, buf),
        }
    }
}
impl AsyncWrite for Link {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        match self.get_mut() {
            Link::Process(pipe, _) => Pin::new(pipe).poll_write(cx, buf),
            Link::Service(pipe) => Pin::new(pipe).poll_write(cx, buf),
        }
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            Link::Process(pipe, _) => Pin::new(pipe).poll_flush(cx),
            Link::Service(pipe) => Pin::new(pipe).poll_flush(cx),
        }
    }
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            Link::Process(pipe, _) => Pin::new(pipe).poll_shutdown(cx),
            Link::Service(pipe) => Pin::new(pipe).poll_shutdown(cx),
        }
    }
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
        // Reply 1 means the helper removed the default routes before stopping Xray.
        for _ in 0..2 {
            self.pipe.write_u8(0).await.map_err(|_| error())?;
            let reply = tokio::time::timeout(Duration::from_secs(8), self.pipe.read_u8())
                .await
                .map_err(|_| error())?
                .map_err(|_| error())?;
            if reply == 1 {
                return Ok(());
            }
        }
        Err(error())
    }
    /// Replace the running core inside this elevated process. Does not prompt again.
    pub async fn reload(&mut self, request: StartTunnel, data: &std::path::Path) -> Result<()> {
        let (geo, geoip, geosite) = crate::xray::geo::snapshot(data)?;
        let envelope = StartRequest {
            tunnel: request,
            geo,
            standby: true,
        };
        self.pipe.write_u8(3).await.map_err(|_| error())?;
        write_frame(
            &mut self.pipe,
            &serde_json::to_vec(&envelope).map_err(|_| error())?,
        )
        .await?;
        write_asset(&mut self.pipe, &geoip, crate::xray::geo::MAX_GEOIP_SIZE).await?;
        write_asset(&mut self.pipe, &geosite, crate::xray::geo::MAX_GEOSITE_SIZE).await?;
        let reply = tokio::time::timeout(Duration::from_secs(120), self.pipe.read_u8())
            .await
            .map_err(|_| error())?
            .map_err(|_| error())?;
        if reply == 1 {
            self.standby = true;
            Ok(())
        } else {
            Err(match reply {
                2 => BackendError::new(
                    "TUN_ADAPTER_TIMEOUT",
                    "Windows did not make the tunnel adapter ready",
                ),
                3 => BackendError::new(
                    "CORE_START_FAILED",
                    "Xray exited before the tunnel was ready",
                ),
                4 => BackendError::new(
                    "ROUTING_CONFLICT",
                    "Windows selected a route outside Lumen. Retry or check competing routes",
                ),
                _ => error(),
            })
        }
    }
    pub fn exited(&self) -> bool {
        self.pipe.exited()
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

// Only the debug smoke test starts a tunnel without standby.
#[cfg(debug_assertions)]
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
    let mut pipe = connect(started_at).await?;
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
    Ok(Helper { pipe, standby })
}

/// The tunnel service when it is installed and built from this binary; it needs
/// no prompt. Installing or updating it asks for administrator permission once.
/// Without the service, a helper process asks for this session only.
async fn connect(started_at: Instant) -> Result<Link> {
    use super::service::Opened;
    let mut opened = super::service::open().await;
    if matches!(opened, Opened::Install) {
        match super::service::install_elevated().await {
            Ok(()) => {
                log::info!(
                    "VPN timing: tunnel service installed {} ms",
                    started_at.elapsed().as_millis()
                );
                opened = super::service::open().await;
            }
            // The user declined: do not ask a second time for a helper.
            Err(failure) if failure.code == "PERMISSION_DENIED" => return Err(failure),
            Err(failure) => {
                log::warn!("Tunnel service installation failed: {}", failure.message);
                opened = Opened::Unavailable;
            }
        }
    }
    match opened {
        Opened::Ready(pipe) => {
            log::info!(
                "VPN timing: tunnel service connected {} ms",
                started_at.elapsed().as_millis()
            );
            Ok(Link::Service(pipe))
        }
        Opened::Install | Opened::Unavailable => {
            log::warn!("Tunnel service unavailable; starting a helper for this session");
            launch_helper(started_at).await
        }
    }
}

async fn launch_helper(started_at: Instant) -> Result<Link> {
    let id = uuid::Uuid::new_v4().to_string();
    let pipe = ServerOptions::new()
        .first_pipe_instance(true)
        .reject_remote_clients(true)
        .create(format!(r"\\.\pipe\lumen-{id}"))
        .map_err(|_| error())?;
    // The helper's own log arrives here. A tunnel without it still works.
    let log_pipe = ServerOptions::new()
        .first_pipe_instance(true)
        .reject_remote_clients(true)
        .access_outbound(false)
        .create(super::helper_log::pipe_name(&id))
        .ok();
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
    let helper_pid = unsafe { GetProcessId(process.0) };
    unsafe {
        if GetNamedPipeClientProcessId(pipe.as_raw_handle() as _, &mut pid) == 0
            || pid != helper_pid
        {
            return Err(error());
        }
    }
    if let Some(log_pipe) = log_pipe {
        super::helper_log::forward(log_pipe, helper_pid);
    }
    Ok(Link::Process(pipe, process))
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
/// The client's installation folder while the service runs its session. The
/// service binary lives apart from Lumen's runtime; every executable taken from
/// there is checked against the compiled-in hashes before it is staged.
static RUNTIME_BASE: std::sync::Mutex<Option<std::path::PathBuf>> = std::sync::Mutex::new(None);

pub(super) fn set_runtime_base(base: Option<std::path::PathBuf>) {
    if let Ok(mut slot) = RUNTIME_BASE.lock() {
        *slot = base;
    }
}

fn resolve_resources() -> Result<crate::runtime::RuntimePaths> {
    let base = RUNTIME_BASE.lock().ok().and_then(|slot| slot.clone());
    crate::runtime::resolve_from(base.as_deref())
}

/// Set once by the service: a stop request ends sessions like a closed pipe,
/// routes first, then Xray.
static SHUTDOWN: std::sync::OnceLock<tokio::sync::watch::Receiver<bool>> =
    std::sync::OnceLock::new();

pub(super) fn watch_shutdown(stop: tokio::sync::watch::Receiver<bool>) {
    let _ = SHUTDOWN.set(stop);
}

async fn shutdown_requested() {
    match SHUTDOWN.get() {
        Some(stop) => {
            let _ = stop.clone().wait_for(|stopping| *stopping).await;
        }
        None => std::future::pending().await,
    }
}
pub async fn run(id: &str, parent: u32) -> Result<()> {
    let worker_started = Instant::now();
    uuid::Uuid::parse_str(id).map_err(|_| error())?;
    let _parent = verify_parent(parent)?;
    super::helper_log::install(id, parent);
    log::info!("Started");
    let result = serve(id, parent, worker_started).await;
    match &result {
        Ok(()) => log::info!(
            "Stopped after {} s",
            worker_started.elapsed().as_secs()
        ),
        Err(failure) => log::warn!(
            "Stopped after {} s: {} ({})",
            worker_started.elapsed().as_secs(),
            failure.code,
            failure.message
        ),
    }
    super::helper_log::finish();
    result
}

async fn serve(id: &str, parent: u32, worker_started: Instant) -> Result<()> {
    let mut pipe = ClientOptions::new()
        .open(format!(r"\\.\pipe\lumen-{id}"))
        .map_err(|_| error())?;
    let mut pid = 0;
    unsafe {
        if GetNamedPipeServerProcessId(pipe.as_raw_handle() as _, &mut pid) == 0 || pid != parent {
            return Err(error());
        }
    }
    serve_session(&mut pipe, worker_started).await
}

/// One tunnel session on an authenticated pipe, in the helper or the service.
pub(super) async fn serve_session<P: AsyncRead + AsyncWrite + Unpin>(
    pipe: &mut P,
    worker_started: Instant,
) -> Result<()> {
    let bytes = tokio::time::timeout(Duration::from_secs(15), read_frame(pipe))
        .await
        .map_err(|_| error())??;
    let envelope: StartRequest = serde_json::from_slice(&bytes).map_err(|_| error())?;
    let result = run_session(pipe, envelope, worker_started).await;
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
            let _ = write_frame(pipe, failure.message.as_bytes()).await;
        }
    }
    result
}

enum SessionEvent {
    Command(std::io::Result<u8>),
    Exited,
    Sweep,
    /// The tunnel service is stopping (Windows shutdown or an update).
    Shutdown,
}

fn reload_status(result: &Result<()>) -> u8 {
    match result {
        Ok(()) => 1,
        Err(error) if error.code == "TUN_ADAPTER_TIMEOUT" => 2,
        Err(error) if error.code == "CORE_START_FAILED" => 3,
        Err(error) if error.code == "ROUTING_CONFLICT" => 4,
        Err(_) => 5,
    }
}

const WINTUN_FAILURE: &str = "Wintun adapter startup failed";

/// Start Xray on a fresh LumenTun. Called only while this helper runs no core,
/// so a LumenTun that is still registered belongs to a core that is gone: Xray
/// would open that dead adapter by name and fail with a Wintun error.
async fn spawn_core(
    core: &std::path::Path,
    bytes: &[u8],
) -> Result<super::process::CoreProcess> {
    let cleanup_at = Instant::now();
    let removed = tokio::task::spawn_blocking(super::network::clear_stale_lumen_adapters)
        .await
        .unwrap_or(0);
    if removed > 0 {
        log::warn!(
            "Removed {removed} stale LumenTun adapter(s) before starting Xray ({} ms)",
            cleanup_at.elapsed().as_millis()
        );
    }
    let spawned = super::process::CoreProcess::spawn(core, bytes).await;
    if let Err(failure) = &spawned {
        log::warn!("Xray could not be launched: {} ({})", failure.code, failure.message);
    }
    spawned
}

/// Remove default routes first. Xray is stopped only after the table confirms
/// that LumenTun no longer owns them.
async fn release_then_stop(child: &mut super::process::CoreProcess) {
    let mut logged = false;
    loop {
        let _ = super::network::release_lumen_routes();
        if super::network::routes_released() {
            let _ = child.kill().await;
            return;
        }
        if !logged {
            log::warn!("Waiting to remove tunnel routes before stopping Xray");
            logged = true;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

async fn retire(
    routes: &mut super::network::TunnelRoutes,
    child: &mut Option<super::process::CoreProcess>,
) -> bool {
    if routes.deactivate().is_err() || !super::network::routes_released() {
        log::warn!("Tunnel routes are still installed; Xray stays running");
        return false;
    }
    if let Some(current) = child.as_mut() {
        if current.kill().await.is_err() {
            log::warn!("Xray did not exit after tunnel routes were removed");
        }
    }
    true
}

async fn wait_for_standby(child: &mut super::process::CoreProcess, port: u16) -> Result<()> {
    for _ in 0..800 {
        if child.try_wait().map_err(|_| error())?.is_some() {
            return Err(BackendError::new(
                "CORE_START_FAILED",
                child
                    .diagnostic_category()
                    .unwrap_or("Xray exited before the tunnel was ready"),
            ));
        }
        if super::network::tunnel_adapter_ready()
            && tokio::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port))
                .await
                .is_ok()
        {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    Err(BackendError::new(
        "TUN_ADAPTER_TIMEOUT",
        "Windows did not make the tunnel adapter ready",
    ))
}

async fn respawn_standby(
    child: &mut Option<super::process::CoreProcess>,
    routes: &mut super::network::TunnelRoutes,
    core: &std::path::Path,
    bytes: &[u8],
    port: u16,
) {
    log::warn!("Xray exited; releasing tunnel routes before restart");
    if routes.deactivate().is_err() || !super::network::routes_released() {
        log::warn!("Tunnel routes remain after Xray exited");
        *child = None;
        return;
    }
    tokio::time::sleep(Duration::from_millis(500)).await;
    match spawn_core(core, bytes).await {
        Ok(spawned) => {
            *child = Some(spawned);
            let ready = match child.as_mut() {
                Some(running) => wait_for_standby(running, port).await.is_ok(),
                None => false,
            };
            if ready {
                if let Ok(fresh) = super::network::TunnelRoutes::new() {
                    *routes = fresh;
                }
                log::warn!("Xray restarted in standby without a new elevation");
            } else if let Some(running) = child.as_mut() {
                log::warn!("Xray standby restart did not become ready");
                let _ = running.kill().await;
                *child = None;
            }
        }
        Err(failure) => {
            log::warn!("Xray standby restart failed: {}", failure.code);
            *child = None;
        }
    }
}

/// Xray restarted after it exited while the session was connected. Reinstall the
/// default routes: the parent still shows a connection, and without them every
/// application silently left the tunnel until a health check gave up on it.
fn restore_active_routes(
    pending: &mut bool,
    routes: &mut super::network::TunnelRoutes,
    child: &Option<super::process::CoreProcess>,
) {
    if !*pending || child.is_none() {
        return;
    }
    match routes.activate() {
        Ok(()) => {
            *pending = false;
            log::warn!("Tunnel routes restored after the Xray restart");
        }
        Err(failure) => {
            log::warn!(
                "Tunnel routes were not restored after the Xray restart: {}",
                failure.code
            );
        }
    }
}

async fn apply_reload<P: AsyncRead + AsyncWrite + Unpin>(
    pipe: &mut P,
    child: &mut Option<super::process::CoreProcess>,
    routes: &mut super::network::TunnelRoutes,
    bytes: &mut Vec<u8>,
    core: &mut std::path::PathBuf,
    proxy_port: &mut u16,
    data: &std::path::Path,
) -> Result<()> {
    let frame = tokio::time::timeout(Duration::from_secs(15), read_frame(pipe))
        .await
        .map_err(|_| error())??;
    let envelope: StartRequest = serde_json::from_slice(&frame).map_err(|_| error())?;
    let (geoip, geosite) = tokio::time::timeout(Duration::from_secs(30), async {
        let ip = read_asset(pipe, crate::xray::geo::MAX_GEOIP_SIZE).await?;
        let site = read_asset(pipe, crate::xray::geo::MAX_GEOSITE_SIZE).await?;
        Ok::<_, BackendError>((ip, site))
    })
    .await
    .map_err(|_| error())??;
    if routes.deactivate().is_err() || !super::network::routes_released() {
        return Err(error());
    }
    if let Some(current) = child.as_mut() {
        current.kill().await.map_err(|_| error())?;
    }
    *child = None;
    let config = crate::xray::config::build_standby(
        &envelope.tunnel.repository,
        &envelope.tunnel.selected,
        envelope.tunnel.proxy_port,
        envelope.tunnel.metrics_port,
    )?;
    let next = serde_json::to_vec(&config).map_err(|_| error())?;
    let bundled = stage_core(
        &envelope.tunnel.core_version,
        data,
        envelope.geo,
        &geoip,
        &geosite,
    )
    .await?;
    // Keep the previous config until the replacement is actually listening.
    // A failed wait used to leave the new process in place and overwrite the
    // bytes Sweep would use for the next restart.
    *child = Some(spawn_core(&bundled, &next).await?);
    let mut ready = {
        let running = child.as_mut().ok_or_else(error)?;
        wait_for_standby(running, envelope.tunnel.proxy_port).await
    };
    // Windows can still be tearing down the previous core's adapter when the
    // new one opens it. Clear it again and retry once on the same elevation.
    if matches!(&ready, Err(failure) if failure.message == WINTUN_FAILURE) {
        log::warn!("Xray could not open LumenTun after reload; retrying after adapter cleanup");
        if let Some(running) = child.as_mut() {
            let _ = running.kill().await;
        }
        *child = None;
        *child = Some(spawn_core(&bundled, &next).await?);
        ready = {
            let running = child.as_mut().ok_or_else(error)?;
            wait_for_standby(running, envelope.tunnel.proxy_port).await
        };
    }
    if let Err(failure) = ready {
        if let Some(running) = child.as_mut() {
            let _ = running.kill().await;
        }
        *child = None;
        return Err(failure);
    }
    let fresh = match super::network::TunnelRoutes::new() {
        Ok(fresh) => fresh,
        Err(failure) => {
            if let Some(running) = child.as_mut() {
                let _ = running.kill().await;
            }
            *child = None;
            return Err(failure);
        }
    };
    *routes = fresh;
    if !super::network::standby_isolated() {
        if let Some(running) = child.as_mut() {
            let _ = running.kill().await;
        }
        *child = None;
        return Err(BackendError::new(
            "TUN_FAILED",
            "Prepared tunnel is not isolated from normal networking",
        ));
    }
    *bytes = next;
    *core = bundled;
    *proxy_port = envelope.tunnel.proxy_port;
    Ok(())
}

/// Stage what the session runs: the bundled Xray and Wintun (checked against the
/// compiled-in hashes), the trusted geo snapshot, then an updated core if one is
/// selected (checked against its release digest, validated with that snapshot).
/// Nothing from the client's runtime folder runs before its hash matched.
async fn stage_core(
    version: &str,
    data: &std::path::Path,
    geo: crate::xray::geo::GeoDatabaseMetadata,
    geoip: &[u8],
    geosite: &[u8],
) -> Result<std::path::PathBuf> {
    let runtime = resolve_resources()?;
    let bundled = crate::xray::bundle::prepare_for_tunnel(&runtime, data)?;
    crate::xray::geo::install_trusted_snapshot(data, geo, geoip, geosite)?;
    if version == crate::xray::bundle::VERSION {
        return Ok(bundled);
    }
    crate::xray::updater::prepare(version, &data.join("core")).await
}

async fn run_session<P: AsyncRead + AsyncWrite + Unpin>(
    mut pipe: &mut P,
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
    let mut bytes = serde_json::to_vec(&config).map_err(|_| error())?;
    let config_ready = worker_started.elapsed().as_millis() as u32;
    let staging = super::staging::Directory::create()?;
    let data = tempfile::tempdir_in(staging.path()).map_err(|_| error())?;
    // The authenticated pipe supplies the Geo snapshot; bundled Geo is not initialized here.
    let mut core = stage_core(
        &request.core_version,
        data.path(),
        envelope.geo,
        &geoip,
        &geosite,
    )
    .await?;
    // ~17 MB that would otherwise stay in the service for the whole session.
    drop((geoip, geosite));
    let core_ready = worker_started.elapsed().as_millis() as u32;
    let geo_ready = core_ready;
    for _ in 0..10 {
        if super::network::release_lumen_routes().is_ok() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    if !super::network::routes_released() {
        log::warn!("Leftover LumenTun routes were still installed at helper start");
    }
    let mut child = spawn_core(&core, &bytes).await?;
    let xray_spawned = worker_started.elapsed().as_millis() as u32;
    // A closed pipe, including a parent crash, removes routes before Xray exits.
    let mut ready = false;
    let mut routes = None;
    let mut proxy_port = request.proxy_port;
    let attempts = if standby { 800 } else { 400 };
    let mut remaining = attempts;
    let mut adapter_retried = false;
    while remaining > 0 {
        remaining -= 1;
        if child.try_wait().map_err(|_| error())?.is_some() {
            let message = child
                .diagnostic_category()
                .unwrap_or("Xray exited before the tunnel was ready");
            tokio::time::sleep(Duration::from_millis(20)).await;
            release_then_stop(&mut child).await;
            // Windows can still be tearing down a dead LumenTun when Xray opens
            // it. Clear it again and retry once without a new elevation.
            if message == WINTUN_FAILURE && !adapter_retried {
                log::warn!("Xray could not open LumenTun; retrying after adapter cleanup");
                adapter_retried = true;
                remaining = attempts;
                child = spawn_core(&core, &bytes).await?;
                continue;
            }
            return Err(BackendError::new("CORE_START_FAILED", message));
        }
        let network_ready = if standby {
            // Xray configures both IP families before opening the SOCKS listener.
            // Do not race its interface setup by changing metrics as soon as the
            // IPv4 adapter appears: Xray can still overwrite them afterwards.
            if super::network::tunnel_adapter_ready()
                && tokio::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, proxy_port))
                    .await
                    .is_ok()
            {
                if routes.is_none() {
                    match super::network::TunnelRoutes::new() {
                        Ok(created) => routes = Some(created),
                        Err(failure) => {
                            release_then_stop(&mut child).await;
                            return Err(failure);
                        }
                    }
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
        // Which half was missing tells a Wintun problem from a core that never
        // opened its local port.
        let port_open = tokio::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, proxy_port))
            .await
            .is_ok();
        log::warn!(
            "LumenTun was not ready after {} ms (adapter up: {}, core port open: {})",
            worker_started.elapsed().as_millis(),
            super::network::tunnel_adapter_ready(),
            port_open
        );
        if !standby && super::network::tunnel_ready() {
            if let Err(failure) = super::network::verify_tunnel_routes() {
                release_then_stop(&mut child).await;
                return Err(failure);
            }
        }
        release_then_stop(&mut child).await;
        return Err(BackendError::new(
            "TUN_ADAPTER_TIMEOUT",
            "Windows did not make the tunnel adapter ready",
        ));
    }
    if standby && !super::network::standby_isolated() {
        release_then_stop(&mut child).await;
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
        let data_path = data.path().to_path_buf();
        let mut child = Some(child);
        // Set when Xray dies while the routes are installed (a connected session).
        let mut restore_routes = false;
        loop {
            let event = if let Some(running) = child.as_mut() {
                tokio::select! {
                    command = pipe.read_u8() => SessionEvent::Command(command),
                    _ = running.wait() => SessionEvent::Exited,
                    _ = shutdown_requested() => SessionEvent::Shutdown,
                }
            } else {
                tokio::select! {
                    command = pipe.read_u8() => SessionEvent::Command(command),
                    _ = tokio::time::sleep(Duration::from_millis(200)) => SessionEvent::Sweep,
                    _ = shutdown_requested() => SessionEvent::Shutdown,
                }
            };
            match event {
                SessionEvent::Exited => {
                    restore_routes |= routes.is_active();
                    respawn_standby(&mut child, routes, &core, &bytes, proxy_port).await;
                    restore_active_routes(&mut restore_routes, routes, &child);
                }
                SessionEvent::Sweep => {
                    if routes.deactivate().is_ok() && super::network::routes_released() {
                        respawn_standby(&mut child, routes, &core, &bytes, proxy_port).await;
                        restore_active_routes(&mut restore_routes, routes, &child);
                    }
                }
                SessionEvent::Command(Ok(1)) => {
                    restore_routes = false;
                    let mut result = routes.activate();
                    for _ in 0..8 {
                        if !matches!(&result, Err(error) if error.code == "TUN_FAILED") {
                            break;
                        }
                        tokio::time::sleep(Duration::from_millis(25)).await;
                        result = routes.activate();
                    }
                    match &result {
                        Ok(()) => log::info!("Routes activated"),
                        Err(failure) => log::warn!(
                            "Route activation failed: {} ({})",
                            failure.code,
                            failure.message
                        ),
                    }
                    let (code, detail) = match result {
                        Ok(()) => (1, None),
                        Err(failure) if failure.code == "ROUTING_CONFLICT" => {
                            (4, Some(failure.message))
                        }
                        Err(failure) if failure.code == "TUN_FAILED" => (5, None),
                        Err(_) => (0, None),
                    };
                    pipe.write_u8(code).await.map_err(|_| error())?;
                    if let Some(detail) = detail {
                        write_frame(&mut pipe, detail.as_bytes()).await?;
                    }
                }
                SessionEvent::Command(Ok(2)) => {
                    restore_routes = false;
                    let ok = routes.deactivate().is_ok() && super::network::routes_released();
                    if ok {
                        log::info!("Routes released");
                    } else {
                        log::warn!("Routes could not be released");
                    }
                    pipe.write_u8(if ok { 1 } else { 0 })
                        .await
                        .map_err(|_| error())?;
                }
                SessionEvent::Command(Ok(3)) => {
                    restore_routes = false;
                    let reload_at = Instant::now();
                    log::info!("Reloading the tunnel core");
                    let result = apply_reload(
                        pipe,
                        &mut child,
                        routes,
                        &mut bytes,
                        &mut core,
                        &mut proxy_port,
                        &data_path,
                    )
                    .await;
                    let code = reload_status(&result);
                    if let Err(failure) = &result {
                        log::warn!(
                            "engine=xray helper reload failed: {} ({})",
                            failure.code,
                            failure.message
                        );
                    } else {
                        log::info!(
                            "Tunnel core reloaded in {} ms",
                            reload_at.elapsed().as_millis()
                        );
                    }
                    pipe.write_u8(code).await.map_err(|_| error())?;
                }
                SessionEvent::Command(Ok(0)) => {
                    log::info!("Stop requested");
                    if retire(routes, &mut child).await {
                        pipe.write_u8(1).await.map_err(|_| error())?;
                        break;
                    }
                    pipe.write_u8(0).await.map_err(|_| error())?;
                }
                SessionEvent::Command(Ok(command)) => {
                    log::warn!("Unknown command {command}");
                    pipe.write_u8(0).await.map_err(|_| error())?;
                }
                SessionEvent::Shutdown => {
                    log::info!("Tunnel service is stopping; releasing routes");
                    for _ in 0..25 {
                        if retire(routes, &mut child).await {
                            break;
                        }
                        tokio::time::sleep(Duration::from_millis(200)).await;
                    }
                    break;
                }
                SessionEvent::Command(Err(_)) => {
                    log::info!("Lumen closed the helper pipe; releasing routes");
                    while !retire(routes, &mut child).await {
                        tokio::time::sleep(Duration::from_millis(200)).await;
                    }
                    break;
                }
            }
        }
    } else {
        tokio::select! {
            _ = pipe.read_u8() => {}
            _ = child.wait() => {}
            _ = shutdown_requested() => {}
        }
        release_then_stop(&mut child).await;
        let _ = pipe.write_u8(1).await;
    }
    Ok(())
}
