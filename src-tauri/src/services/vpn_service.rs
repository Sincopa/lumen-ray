use crate::{
    app::state::AppState,
    domain::errors::{BackendError, Result},
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    process::Stdio,
    sync::{atomic::Ordering, Arc},
    time::{Duration, Instant},
};
use tauri::Emitter;
use tokio::{io::AsyncWriteExt, process::Command};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionInfo {
    pub server_id: String,
    pub ip: String,
    pub protocol: String,
    pub connected_at: u64,
    pub demo: bool,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub status: String,
    pub info: Option<ConnectionInfo>,
    pub error: Option<BackendError>,
}
impl Default for Snapshot {
    fn default() -> Self {
        Self {
            status: "disconnected".into(),
            info: None,
            error: None,
        }
    }
}
#[derive(Default)]
pub struct Session {
    #[cfg(windows)]
    pub child: Option<crate::platform::process::CoreProcess>,
    #[cfg(not(windows))]
    pub child: Option<tokio::process::Child>,
    #[cfg(windows)]
    pub helper: Option<crate::platform::helper::Helper>,
    #[cfg(windows)]
    pub system_proxy: Option<crate::platform::proxy::SystemProxy>,
    pub proxy_port: u16,
    pub metrics_port: u16,
    pub started: Option<Instant>,
    #[cfg(windows)]
    pub prepared_server_id: Option<String>,
    #[cfg(windows)]
    pub prepared_config_hash: Option<[u8; 32]>,
    #[cfg(windows)]
    pub prepared_core_version: Option<String>,
    #[cfg(windows)]
    pub prepared_geo_hash: Option<[u8; 32]>,
}
pub fn command(core: &std::path::Path) -> Command {
    let mut command = Command::new(core);
    if let Some(parent) = core.parent() {
        command.current_dir(parent);
        command.env("XRAY_LOCATION_ASSET", crate::xray::geo::assets_for_core(core));
    } else {
        command.current_dir(core);
    }
    command.env("GOMEMLIMIT", "100MiB");
    command.env("GODEBUG", "madvdontneed=1");
    command.kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    command
}
pub async fn validate(core: &std::path::Path, config: &[u8]) -> Result<()> {
    let mut child = command(core)
        .args(["run", "-test", "-config", "stdin:"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| {
            BackendError::new("CORE_START_FAILED", "Could not run Xray config validation")
        })?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(config).await.map_err(|_| {
            BackendError::new("INVALID_CONFIG", "Could not pass configuration to Xray")
        })?;
    }
    let output = tokio::time::timeout(Duration::from_secs(10), child.wait_with_output())
        .await
        .map_err(|_| BackendError::new("INVALID_CONFIG", "Xray validation timed out"))?
        .map_err(|_| BackendError::new("INVALID_CONFIG", "Xray validation failed"))?;
    if !output.status.success() {
        let output = format!(
            "{} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let failure = output
            .lines()
            .find(|line| line.contains("Failed to start"))
            .unwrap_or(&output);
        if failure.contains("failed to open geoip.dat")
            || failure.contains("failed to open geosite.dat")
            || failure.contains("failed to check code")
            || failure.contains("illegal ip rule: geoip:")
            || failure.contains("illegal domain rule: geosite:")
        {
            return Err(BackendError::new(
                "GEO_DATA_MISSING",
                "GeoIP/GeoSite database files are missing or corrupted. Please update databases in Settings.",
            ));
        }
        let category = [
            "fingerprint",
            "password",
            "shortId",
            "serverName",
            "flow",
            "ALPN",
            "alpn",
            "encryption",
            "security",
            "Vision",
            "REALITY",
            "routing",
            "Host",
            "headers",
            "removed",
            "WebSocket",
            "XHTTP",
        ]
        .into_iter()
        .find(|s| failure.contains(s))
        .unwrap_or("configuration");
        return Err(BackendError::new(
            "INVALID_CONFIG",
            format!("Xray rejected {category} parameters"),
        ));
    }
    Ok(())
}
pub(crate) fn ports() -> Result<(u16, u16)> {
    let error = || BackendError::new("CORE_START_FAILED", "No local ports available");
    // Hold reservations together: Windows may reuse a just-released ephemeral port.
    let proxy = std::net::TcpListener::bind("127.0.0.1:0").map_err(|_| error())?;
    for _ in 0..32 {
        let metrics = std::net::TcpListener::bind("127.0.0.1:0").map_err(|_| error())?;
        let port = metrics.local_addr().map_err(|_| error())?.port();
        if let (Some(http), Some(api)) = (port.checked_add(1), port.checked_add(2)) {
            if let (Ok(_http), Ok(_api)) = (
                std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, http)),
                std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, api)),
            ) {
                return Ok((proxy.local_addr().map_err(|_| error())?.port(), port));
            }
        }
    }
    Err(error())
}
pub async fn publish(state: &AppState, app: &tauri::AppHandle, snapshot: Snapshot) {
    *state.snapshot.write().await = snapshot.clone();
    if app.emit("vpn://state", snapshot).is_err() {
        log::warn!("Unable to deliver connection state event");
    }
    crate::update_tray_menu(app, state);
}
pub async fn connect(state: &Arc<AppState>, app: &tauri::AppHandle, id: String) -> Result<()> {
    connect_with_selection(state, app, id, true).await
}
pub async fn connect_exact(
    state: &Arc<AppState>,
    app: &tauri::AppHandle,
    id: String,
) -> Result<()> {
    connect_with_selection(state, app, id, false).await
}

/// Prepare the selected TUN without installing system routes. Failure leaves the
/// normal on-demand connection path available.
pub async fn prewarm(state: &Arc<AppState>) {
    #[cfg(windows)]
    {
        let Ok(_operation) = state.connection_operation.acquire().await else { return; };
        if *state.shutdown.borrow() { return; }
        let repo = state.repository.read().await.clone();
        if repo.settings.network_mode != "tun" || repo.settings.auto_connect || repo.selected_server_id.is_none() {
            let mut session = state.session.lock().await;
            if session.started.is_none() && session.helper.is_some() {
                if let Err(error) = stop(&mut session).await {
                    log::warn!("VPN TUN preparation cleanup failed: {}", error.code);
                }
            }
            return;
        }
        let Some(selected) = repo.selected_server_id.clone() else { return; };
        let selected = connection_candidates(state, &selected).await.into_iter().next().unwrap_or(selected);
        let Ok(config_hash) = configuration_fingerprint(&repo, &selected) else { return; };
        let geo_hash = geo_fingerprint(&state.data);
        let Ok((proxy_port, metrics_port)) = ports() else { return; };
        let core = state.core.read().await.clone();
        let version = core.parent().and_then(|p| p.file_name()).and_then(|s| s.to_str())
            .unwrap_or(crate::xray::core::VERSION).to_owned();
        let mut session = state.session.lock().await;
        if session.started.is_some() { return; }
        if session.helper.is_some() {
            if session.prepared_server_id.as_deref() == Some(&selected)
                && session.prepared_config_hash == Some(config_hash)
                && session.prepared_geo_hash == geo_hash
                && session.helper.as_ref().is_some_and(|helper| !helper.exited()) {
                return;
            }
            if let Err(error) = stop(&mut session).await {
                log::warn!("VPN TUN preparation cleanup failed: {}", error.code);
                return;
            }
        }
        let mut repository = repo;
        repository.sources.clear();
        let request = crate::platform::helper::StartTunnel {
            core_version: version,
            repository,
            selected: selected.clone(),
            proxy_port,
            metrics_port,
        };
        let mut prepared = crate::platform::helper::start_standby(request.clone(), &state.data).await;
        if prepared.as_ref().err().is_some_and(|error|
            ["TUN_ADAPTER_TIMEOUT", "CORE_START_FAILED", "TUN_FAILED"].contains(&error.code)) {
            log::warn!("VPN TUN preparation retry after a transient adapter failure");
            tokio::time::sleep(Duration::from_millis(200)).await;
            prepared = crate::platform::helper::start_standby(request, &state.data).await;
        }
        match prepared {
            Ok(helper) => {
                session.helper = Some(helper);
                session.prepared_server_id = Some(selected);
                session.prepared_config_hash = Some(config_hash);
                session.prepared_core_version = Some(core.parent().and_then(|p| p.file_name())
                    .and_then(|s| s.to_str()).unwrap_or(crate::xray::core::VERSION).to_owned());
                session.prepared_geo_hash = geo_hash;
                session.proxy_port = proxy_port;
                session.metrics_port = metrics_port;
                log::info!("VPN TUN prepared without installing routes");
            }
            Err(error) => log::warn!("VPN TUN preparation failed: {}", error.code),
        }
    }
}

#[cfg(windows)]
fn configuration_fingerprint(repo: &crate::storage::repository::Repository, selected: &str) -> Result<[u8; 32]> {
    let config = crate::xray::config::build(repo, selected, 10808, 10809, true)?;
    let mut hasher = Sha256::new();
    serde_json::to_writer(&mut hasher, &config)
        .map_err(|_| BackendError::new("INVALID_CONFIG", "Could not serialize Xray config"))?;
    Ok(hasher.finalize().into())
}

#[cfg(windows)]
fn geo_fingerprint(data: &std::path::Path) -> Option<[u8; 32]> {
    std::fs::read(crate::xray::geo::metadata_path(data))
        .ok().map(|bytes| Sha256::digest(bytes).into())
}

/// Apply a routing-only catalog change without destroying the active TUN.
/// Returns false when the change needs a full core restart.
#[cfg(windows)]
pub async fn apply_live_rules(
    state: &Arc<AppState>,
    selected: &str,
    before: &crate::storage::repository::Repository,
    after: &crate::storage::repository::Repository,
) -> Result<bool> {
    let _operation = state.connection_operation.acquire().await
        .map_err(|_| BackendError::new("BUSY", "Connection manager is shutting down"))?;
    let mut session = state.session.lock().await;
    let after_hash = configuration_fingerprint(after, selected)?;
    if session.started.is_some()
        && session.prepared_server_id.as_deref() == Some(selected)
        && session.prepared_config_hash == Some(after_hash)
        && session.helper.as_ref().is_some_and(|helper| !helper.exited())
    {
        return Ok(true);
    }
    if session.started.is_none()
        || session.helper.as_ref().is_none_or(|helper| helper.exited())
        || session.prepared_server_id.as_deref() != Some(selected)
        || session.prepared_config_hash != Some(configuration_fingerprint(before, selected)?)
    {
        return Ok(false);
    }
    let Some(patch) = crate::xray::config::live_routing_patch(before, after, selected)? else {
        return Ok(false);
    };
    let core = state.core.read().await.clone();
    run_live_routing_patch(&core, session.metrics_port + 2, &patch).await?;
    session.prepared_config_hash = Some(after_hash);
    Ok(true)
}

#[cfg(windows)]
pub(crate) async fn run_live_routing_patch(core: &std::path::Path, api_port: u16, patch: &serde_json::Value) -> Result<()> {
    let mut command = self::command(core);
    command.args(["api", "adrules", &format!("--server=127.0.0.1:{api_port}"), "-timeout=5", "stdin:"]);
    command.stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null());
    let mut child = command.spawn().map_err(|_| BackendError::new("RULE_UPDATE_FAILED", "Could not start Xray routing update"))?;
    let bytes = serde_json::to_vec(&patch)
        .map_err(|_| BackendError::new("RULE_UPDATE_FAILED", "Could not encode routing update"))?;
    if let Some(mut input) = child.stdin.take() {
        input.write_all(&bytes).await.map_err(|_| BackendError::new("RULE_UPDATE_FAILED", "Could not send routing update"))?;
    }
    let status = tokio::time::timeout(Duration::from_secs(8), child.wait()).await
        .map_err(|_| BackendError::new("RULE_UPDATE_FAILED", "Xray routing update timed out"))?
        .map_err(|_| BackendError::new("RULE_UPDATE_FAILED", "Xray routing update failed"))?;
    if !status.success() {
        return Err(BackendError::new("RULE_UPDATE_FAILED", "Xray rejected the routing update"));
    }
    Ok(())
}
async fn connect_with_selection(
    state: &Arc<AppState>,
    app: &tauri::AppHandle,
    id: String,
    best: bool,
) -> Result<()> {
    let requested_at = Instant::now();
    let _operation = state
        .connection_operation
        .acquire()
        .await
        .map_err(|_| BackendError::new("BUSY", "Connection manager is shutting down"))?;
    publish(
        state,
        app,
        Snapshot { status: "connecting".into(), info: None, error: None },
    ).await;
    let mut session = std::mem::take(&mut *state.session.lock().await);
    let candidates = if best {
        connection_candidates(state, &id).await
    } else {
        vec![id]
    };
    log::info!("VPN timing: candidate selection {} ms", requested_at.elapsed().as_millis());
    let mut result = Err(BackendError::new("INVALID_SERVER", "No available server"));
    for candidate in candidates {
        result = connect_session(state, app, candidate.clone(), &mut session).await;
        if result.as_ref().err().is_some_and(|error|
            error.code == "TUN_ADAPTER_TIMEOUT"
                || (error.code == "CORE_START_FAILED"
                    && ["Wintun adapter startup failed", "Windows tunnel interface is not ready"].contains(&error.message.as_str()))) {
            log::warn!("Windows TUN adapter startup was transient; retrying once");
            result = connect_session(state, app, candidate.clone(), &mut session).await;
        }
        if result.is_ok() {
            *state.desired_server.write().await = Some(candidate.clone());
            let selected = state.repository.read().await.selected_server_id.clone();
            if selected.as_deref() != Some(&candidate) {
                if let Ok(catalog) = super::catalog_service::mutate(
                    state,
                    super::catalog_service::Mutation::SelectServer { id: candidate },
                )
                .await
                {
                    let _ = app.emit("catalog://changed", catalog);
                }
            }
            break;
        }
        if !["CORE_START_FAILED", "SERVER_UNREACHABLE", "CORE_CRASHED"]
            .contains(&result.as_ref().err().map(|error| error.code).unwrap_or(""))
        {
            break;
        }
    }
    *state.session.lock().await = session;
    if let Err(error) = &result {
        publish(state, app, Snapshot {
            status: "error".into(), info: None, error: Some(error.clone()),
        }).await;
    }
    log::info!("VPN timing: connection result {} ms", requested_at.elapsed().as_millis());
    result
}
async fn connection_candidates(state: &Arc<AppState>, selected: &str) -> Vec<String> {
    let repo = state.repository.read().await;
    if repo.settings.server_selection != "best" {
        return vec![selected.to_owned()];
    }
    let Some(node) = repo.nodes.iter().find(|node| node.id == selected) else {
        return vec![selected.to_owned()];
    };
    let mut choices: Vec<_> = repo
        .nodes
        .iter()
        .filter(|candidate| candidate.subscription_id == node.subscription_id)
        .map(|node| node.id.clone())
        .collect();
    drop(repo);
    choices.truncate(16);
    if !choices.iter().any(|id| id == selected) {
        choices.pop();
        choices.push(selected.to_owned());
    }
    rank_candidates(choices, selected, &*state.latencies.read().await)
}
fn rank_candidates(
    mut choices: Vec<String>,
    selected: &str,
    latencies: &std::collections::HashMap<String, serde_json::Value>,
) -> Vec<String> {
    choices.sort_by_key(|id| {
        (
            latencies
                .get(id)
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(u64::MAX),
            if id == selected { 0 } else { 1 },
        )
    });
    choices.truncate(8);
    if !choices.iter().any(|id| id == selected) {
        choices.pop();
        choices.push(selected.to_owned());
    }
    choices
}
async fn connect_session(
    state: &Arc<AppState>,
    app: &tauri::AppHandle,
    id: String,
    session: &mut Session,
) -> Result<()> {
    let session_at = Instant::now();
    let repo = state.repository.read().await.clone();
    let uses_geo = repo.routing_mode == "rules" && repo.rules.iter().any(|rule| {
        rule.enabled && matches!(rule.matcher,
            crate::storage::repository::RuleMatcher::Geosite |
            crate::storage::repository::RuleMatcher::Geoip)
    });
    if uses_geo && !crate::xray::geo::info(&state.data).valid {
        return Err(BackendError::new("GEO_DATA_CORRUPTED", "Geo databases must pass validation before connecting"));
    }
    #[cfg(windows)]
    let config_hash = configuration_fingerprint(&repo, &id)?;
    let core = state.core.read().await.clone();
    #[cfg(windows)]
    let core_version = core.parent().and_then(|p| p.file_name()).and_then(|s| s.to_str())
        .unwrap_or(crate::xray::core::VERSION);
    #[cfg(windows)]
    let reuse_prepared = session.started.is_none()
        && session.prepared_server_id.as_deref() == Some(&id)
        && session.prepared_config_hash == Some(config_hash)
        && session.prepared_core_version.as_deref() == Some(core_version)
        && session.prepared_geo_hash == geo_fingerprint(&state.data)
        && session.helper.as_ref().is_some_and(|h| !h.exited() && h.is_standby());
    #[cfg(not(windows))]
    let reuse_prepared = false;
    let (proxy_port, metrics_port) = if reuse_prepared {
        (session.proxy_port, session.metrics_port)
    } else {
        ports()?
    };
    let is_tun = repo.settings.network_mode == "tun";
    let bytes = if is_tun {
        #[cfg(not(windows))]
        let _ = crate::xray::config::build(&repo, &id, proxy_port, metrics_port, true)?;
        Vec::new()
    } else {
        let config = crate::xray::config::build(&repo, &id, proxy_port, metrics_port, false)?;
        serde_json::to_vec(&config)
            .map_err(|_| BackendError::new("INVALID_CONFIG", "Could not serialize Xray config"))?
    };
    log::info!("VPN timing: config prepared {} ms", session_at.elapsed().as_millis());
    if !reuse_prepared { stop(session).await?; }
    log::info!("VPN timing: previous session stopped {} ms", session_at.elapsed().as_millis());
    let mut result = if is_tun {
        start_tunnel(
            session,
            &state.data,
            repo.clone(),
            id.clone(),
            (proxy_port, metrics_port),
            core.parent()
                .and_then(|p| p.file_name())
                .and_then(|s| s.to_str())
                .unwrap_or(crate::xray::core::VERSION)
                .into(),
            #[cfg(windows)]
            config_hash,
        )
        .await
    } else {
        start(&core, session, &bytes, proxy_port, metrics_port).await
    };
    log::info!("VPN timing: local route startup completed {} ms", session_at.elapsed().as_millis());
    #[cfg(windows)]
    if result.is_ok() && repo.settings.network_mode == "proxy" {
        match crate::platform::proxy::SystemProxy::enable(&state.data, metrics_port + 1) {
            Ok(proxy) => session.system_proxy = Some(proxy),
            Err(error) => result = Err(error),
        }
    }
    log::info!("VPN timing: system proxy stage completed {} ms", session_at.elapsed().as_millis());
    match result {
        Ok(()) => {
            let node = repo
                .nodes
                .iter()
                .find(|n| n.id == id)
                .ok_or_else(BackendError::invalid)?;
            let protocol = match node.protocol {
                crate::domain::node::Protocol::Vless => "Vless",
                crate::domain::node::Protocol::Vmess => "VMess",
                crate::domain::node::Protocol::Trojan => "Trojan",
                crate::domain::node::Protocol::Shadowsocks => "Shadowsocks",
                crate::domain::node::Protocol::Hysteria => "Hysteria",
                crate::domain::node::Protocol::Wireguard => "WireGuard",
            };
            publish(
                state,
                app,
                Snapshot {
                    status: "connected".into(),
                    info: Some(ConnectionInfo {
                        server_id: id.clone(),
                        ip: String::new(),
                        protocol: protocol.into(),
                        connected_at: super::catalog_service::now(),
                        demo: false,
                    }),
                    error: None,
                },
            )
            .await;
            log::info!("VPN timing: connected published {} ms", session_at.elapsed().as_millis());
            refresh_public_ip(state.clone(), app.clone(), id, proxy_port);
            Ok(())
        }
        Err(error) => {
            let cleanup = stop(session).await;
            cleanup?;
            Err(error)
        }
    }
}
async fn start_tunnel(
    session: &mut Session,
    data: &std::path::Path,
    mut repository: crate::storage::repository::Repository,
    selected: String,
    (proxy_port, metrics_port): (u16, u16),
    core_version: String,
    #[cfg(windows)] config_hash: [u8; 32],
) -> Result<()> {
    repository.sources.clear();
    let tunnel_at = Instant::now();
    #[cfg(windows)]
    {
        if session.helper.is_none() {
            session.helper = Some(crate::platform::helper::start_standby(crate::platform::helper::StartTunnel {
                core_version: core_version.clone(),
                repository,
                selected: selected.clone(),
                proxy_port,
                metrics_port,
            }, data)
            .await?);
        }
        session.helper.as_mut().ok_or_else(BackendError::invalid)?.activate().await?;
        session.prepared_server_id = Some(selected);
        session.prepared_config_hash = Some(config_hash);
        session.prepared_core_version = Some(core_version);
        session.prepared_geo_hash = geo_fingerprint(data);
        session.proxy_port = proxy_port;
        session.metrics_port = metrics_port;
        session.started = Some(Instant::now());
        log::info!("VPN timing: TUN helper ready {} ms", tunnel_at.elapsed().as_millis());
        let mut port_ready = false;
        for _ in 0..100 {
            if session.helper.as_ref().is_some_and(|h| h.exited()) {
                return Err(BackendError::new("TUN_FAILED", "Tunnel helper exited"));
            }
            if tokio::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, proxy_port))
                .await
                .is_ok()
            {
                port_ready = true;
                if crate::platform::network::verify_tunnel_routes().is_ok() {
                    log::info!("VPN timing: TUN local port and routes ready {} ms", tunnel_at.elapsed().as_millis());
                    return Ok(());
                }
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        if port_ready {
            crate::platform::network::verify_tunnel_routes()?;
        }
    }
    Err(BackendError::new("TUN_FAILED", "Tunnel was not ready"))
}
pub(crate) async fn start(
    core: &std::path::Path,
    session: &mut Session,
    bytes: &[u8],
    proxy_port: u16,
    metrics_port: u16,
) -> Result<()> {
    let core_at = Instant::now();
    #[cfg(windows)]
    {
        session.child = Some(crate::platform::process::CoreProcess::spawn(core, bytes).await?);
        log::info!("VPN timing: Xray spawned {} ms", core_at.elapsed().as_millis());
    }
    #[cfg(not(windows))]
    return Err(BackendError::new(
        "PLATFORM_UNSUPPORTED",
        "This build supports Windows networking only",
    ));
    session.proxy_port = proxy_port;
    session.metrics_port = metrics_port;
    session.started = Some(Instant::now());
    for _ in 0..100 {
        if session
            .child
            .as_mut()
            .ok_or_else(BackendError::invalid)?
            .try_wait()
            .map_err(|_| BackendError::new("CORE_CRASHED", "Could not inspect Xray process"))?
            .is_some()
        {
            return Err(BackendError::new(
                "CORE_START_FAILED",
                "Xray exited during startup; TUN requires administrator privileges",
            ));
        }
        if tokio::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, proxy_port))
            .await
            .is_ok()
        {
            log::info!("VPN timing: Xray local port ready {} ms", core_at.elapsed().as_millis());
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    Err(BackendError::new(
        "CORE_START_FAILED",
        "Xray did not become ready",
    ))
}

fn refresh_public_ip(
    state: Arc<AppState>,
    app: tauri::AppHandle,
    server_id: String,
    proxy_port: u16,
) {
    tauri::async_runtime::spawn(async move {
        let network_at = Instant::now();
        let Ok(ip) = public_ip(proxy_port).await else {
            log::info!("VPN timing: public network probe failed after {} ms", network_at.elapsed().as_millis());
            return;
        };
        log::info!("VPN timing: public network probe ready {} ms", network_at.elapsed().as_millis());
        let mut snapshot = state.snapshot.read().await.clone();
        let Some(info) = snapshot.info.as_mut() else {
            return;
        };
        if snapshot.status != "connected" || info.server_id != server_id {
            return;
        }
        info.ip = ip;
        publish(&state, &app, snapshot).await;
    });
}

pub async fn public_ip(proxy_port: u16) -> Result<String> {
    let error = || {
        BackendError::new(
            "SERVER_UNREACHABLE",
            "Could not verify Internet connectivity through the selected server",
        )
    };
    let proxy =
        reqwest::Proxy::all(format!("socks5h://127.0.0.1:{proxy_port}")).map_err(|_| error())?;
    let client = reqwest::Client::builder()
        .no_proxy()
        .proxy(proxy)
        .timeout(Duration::from_millis(2000))
        .build()
        .map_err(|_| error())?;

    for endpoint in [
        "https://api.ipify.org",
        "https://icanhazip.com",
        "https://checkip.amazonaws.com",
        "http://cp.cloudflare.com/generate_204",
    ] {
        if let Ok(response) = client.get(endpoint).send().await {
            if response.status().is_success() || response.status().as_u16() == 204 {
                if let Ok(text) = response.text().await {
                    let cleaned = text.trim();
                    if cleaned.parse::<std::net::IpAddr>().is_ok() {
                        return Ok(cleaned.to_string());
                    }
                }
                return Ok("Connected".into());
            }
        }
    }

    Err(error())
}
pub async fn stop(session: &mut Session) -> Result<()> {
    #[cfg(windows)]
    if let Some(proxy) = session.system_proxy.as_ref() {
        proxy.restore()?;
        session.system_proxy = None;
    }
    #[cfg(windows)]
    if let Some(mut helper) = session.helper.take() {
        if !helper.exited() {
            helper.stop().await?;
        }
    }
    #[cfg(windows)]
    { session.prepared_server_id = None; session.prepared_config_hash = None; session.prepared_core_version = None; session.prepared_geo_hash = None; }
    if let Some(mut child) = session.child.take() {
        if child
            .try_wait()
            .map_err(|_| BackendError::new("CORE_STOP_FAILED", "Cannot inspect Xray process"))?
            .is_none()
        {
            child
                .kill()
                .await
                .map_err(|_| BackendError::new("CORE_STOP_FAILED", "Cannot stop Xray process"))?;
        }
    }
    session.started = None;
    Ok(())
}
pub async fn disconnect(state: &AppState, app: &tauri::AppHandle) -> Result<()> {
    *state.desired_server.write().await = None;
    let _operation = state
        .connection_operation
        .acquire()
        .await
        .map_err(|_| BackendError::new("BUSY", "Connection manager is shutting down"))?;
    let mut session = std::mem::take(&mut *state.session.lock().await);
    let result = disconnect_session(state, app, &mut session).await;
    *state.session.lock().await = session;
    result
}
async fn disconnect_session(
    state: &AppState,
    app: &tauri::AppHandle,
    session: &mut Session,
) -> Result<()> {
    publish(
        state,
        app,
        Snapshot {
            status: "disconnecting".into(),
            info: None,
            error: None,
        },
    )
    .await;
    let shutdown = if let Some(helper) = session.helper.as_mut().filter(|h| h.is_standby() && !h.exited()) {
        let result = if session.started.is_some() { helper.deactivate().await } else { Ok(()) };
        if result.is_ok() { session.started = None; }
        result
    } else { stop(session).await };
    if let Err(error) = shutdown {
        let _ = stop(session).await;
        publish(
            state,
            app,
            Snapshot {
                status: "error".into(),
                info: None,
                error: Some(error.clone()),
            },
        )
        .await;
        return Err(error);
    }
    *state.traffic.write().await = Default::default();
    publish(state, app, Snapshot::default()).await;
    Ok(())
}
pub(super) fn schedule_recovery(state: Arc<AppState>, app: tauri::AppHandle, server_id: String) {
    if state.recovery_running.swap(true, Ordering::AcqRel) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        let mut delay = 2u64;
        loop {
            if *state.shutdown.borrow()
                || state.desired_server.read().await.as_deref() != Some(&server_id)
                || !state.repository.read().await.settings.auto_reconnect
            {
                break;
            }
            tokio::time::sleep(Duration::from_secs(delay)).await;
            if *state.shutdown.borrow()
                || state.desired_server.read().await.as_deref() != Some(&server_id)
                || !state.repository.read().await.settings.auto_reconnect
            {
                break;
            }
            if state.snapshot.read().await.status == "connected" {
                break;
            }
            match connect(&state, &app, server_id.clone()).await {
                Ok(()) => break,
                Err(error) if ["BUSY", "ROUTING_CONFLICT"].contains(&error.code) => {}
                Err(_) => {}
            }
            delay = (delay * 2).min(60);
        }
        state.recovery_running.store(false, Ordering::Release);
    });
}
pub fn monitor(state: Arc<AppState>, app: tauri::AppHandle) {
    super::vpn_monitor::monitor(state, app);
}

#[cfg(test)]
mod selection_tests {
    #[test]
    fn ranks_numeric_latency_first_and_keeps_selected_fallback() {
        let latencies = std::collections::HashMap::from([
            ("slow".to_string(), serde_json::json!(120)),
            ("fast".to_string(), serde_json::json!(28)),
            ("down".to_string(), serde_json::json!("timeout")),
        ]);
        let choices = vec!["slow", "down", "selected", "fast"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        assert_eq!(
            super::rank_candidates(choices, "selected", &latencies),
            ["fast", "slow", "selected", "down"]
        );
    }

    #[test]
    fn missing_latency_cache_keeps_selected_first_without_probing() {
        let choices = vec!["other".to_owned(), "selected".to_owned()];
        assert_eq!(
            super::rank_candidates(choices, "selected", &Default::default()),
            ["selected", "other"]
        );
    }
}
