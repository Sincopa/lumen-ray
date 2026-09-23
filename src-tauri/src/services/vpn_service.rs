use crate::{
    app::state::AppState,
    domain::errors::{BackendError, Result},
};
use serde::Serialize;
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
}
pub fn command(core: &std::path::Path) -> Command {
    let mut command = Command::new(core);
    if let Some(parent) = core.parent() {
        command.current_dir(parent);
        command.env("XRAY_LOCATION_ASSET", parent);
    } else {
        command.current_dir(core);
    }
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
        if let Some(http) = port.checked_add(1) {
            if let Ok(_http) = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, http)) {
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
async fn connect_with_selection(
    state: &Arc<AppState>,
    app: &tauri::AppHandle,
    id: String,
    best: bool,
) -> Result<()> {
    let _operation = state
        .connection_operation
        .try_acquire()
        .map_err(|_| BackendError::new("BUSY", "A connection operation is already running"))?;
    let mut session = std::mem::take(&mut *state.session.lock().await);
    let candidates = if best {
        connection_candidates(state, &id).await
    } else {
        vec![id]
    };
    let mut result = Err(BackendError::new("INVALID_SERVER", "No available server"));
    for candidate in candidates {
        result = connect_session(state, app, candidate.clone(), &mut session, best).await;
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
    let missing: Vec<_> = {
        let latencies = state.latencies.read().await;
        choices
            .iter()
            .filter(|id| {
                latencies
                    .get(*id)
                    .and_then(serde_json::Value::as_u64)
                    .is_none()
            })
            .cloned()
            .collect()
    };
    if !missing.is_empty() {
        let mut probes = tokio::task::JoinSet::new();
        let permits = Arc::new(tokio::sync::Semaphore::new(4));
        for id in missing {
            let state = state.clone();
            let permits = permits.clone();
            probes.spawn(async move {
                if let Ok(_permit) = permits.acquire_owned().await {
                    let _ = super::latency_service::ping(&state, id).await;
                }
            });
        }
        while probes.join_next().await.is_some() {}
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
    choices
}
async fn connect_session(
    state: &Arc<AppState>,
    app: &tauri::AppHandle,
    id: String,
    session: &mut Session,
    verify_best: bool,
) -> Result<()> {
    let repo = state.repository.read().await.clone();
    let core = state.core.read().await.clone();
    if repo.routing_mode == "rules" {
        let has_geo = repo.rules.iter().any(|r| {
            r.enabled
                && matches!(
                    r.matcher,
                    crate::storage::repository::RuleMatcher::Geosite
                        | crate::storage::repository::RuleMatcher::Geoip
                )
        });
        if has_geo {
            if let Some(core_dir) = core.parent() {
                let geoip = core_dir.join("geoip.dat");
                let geosite = core_dir.join("geosite.dat");
                let geo_dir = state.data.join("geo");

                // Self-healing: if files are missing in core_dir but present in data/geo/, restore them
                if (!geoip.is_file() || std::fs::metadata(&geoip).is_ok_and(|m| m.len() < 1024))
                    && geo_dir.join("geoip.dat").is_file()
                {
                    let _ = std::fs::copy(geo_dir.join("geoip.dat"), &geoip);
                }
                if (!geosite.is_file() || std::fs::metadata(&geosite).is_ok_and(|m| m.len() < 1024))
                    && geo_dir.join("geosite.dat").is_file()
                {
                    let _ = std::fs::copy(geo_dir.join("geosite.dat"), &geosite);
                }

                if !geoip.is_file() || !geosite.is_file() {
                    return Err(BackendError::new(
                        "GEO_DATA_MISSING",
                        "GeoIP/GeoSite databases are missing. Please update databases in Settings.",
                    ));
                }
                if std::fs::metadata(&geoip)
                    .map(|m| m.len() < 1024)
                    .unwrap_or(true)
                    || std::fs::metadata(&geosite)
                        .map(|m| m.len() < 1024)
                        .unwrap_or(true)
                {
                    return Err(BackendError::new(
                        "GEO_DATA_CORRUPTED",
                        "GeoIP/GeoSite databases are corrupted. Please update databases in Settings.",
                    ));
                }
                std::env::set_var("XRAY_LOCATION_ASSET", core_dir);
            }
        }
    }
    let (proxy_port, metrics_port) = ports()?;
    let config = crate::xray::config::build(
        &repo,
        &id,
        proxy_port,
        metrics_port,
        repo.settings.network_mode == "tun",
    )?;
    let bytes = serde_json::to_vec(&config)
        .map_err(|_| BackendError::new("INVALID_CONFIG", "Could not serialize Xray config"))?;
    let core = state.core.read().await.clone();
    stop(session).await?;
    publish(
        state,
        app,
        Snapshot {
            status: "connecting".into(),
            info: None,
            error: None,
        },
    )
    .await;
    let mut result = if repo.settings.network_mode == "tun" {
        start_tunnel(
            session,
            repo.clone(),
            id.clone(),
            proxy_port,
            metrics_port,
            core.parent()
                .and_then(|p| p.file_name())
                .and_then(|s| s.to_str())
                .unwrap_or(crate::xray::core::VERSION)
                .into(),
        )
        .await
    } else {
        start(&core, session, &bytes, proxy_port, metrics_port).await
    };
    if result.is_ok()
        && verify_best
        && repo.settings.server_selection == "best"
        && public_ip(proxy_port).await.is_err()
    {
        result = Err(BackendError::new(
            "SERVER_UNREACHABLE",
            "Selected server did not pass the connectivity check",
        ));
    }
    #[cfg(windows)]
    if result.is_ok() && repo.settings.network_mode == "proxy" {
        match crate::platform::proxy::SystemProxy::enable(&state.data, metrics_port + 1) {
            Ok(proxy) => session.system_proxy = Some(proxy),
            Err(error) => result = Err(error),
        }
    }
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
            refresh_public_ip(state.clone(), app.clone(), id, proxy_port);
            Ok(())
        }
        Err(error) => {
            let cleanup = stop(session).await;
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
            cleanup?;
            Err(error)
        }
    }
}
async fn start_tunnel(
    session: &mut Session,
    mut repository: crate::storage::repository::Repository,
    selected: String,
    proxy_port: u16,
    metrics_port: u16,
    core_version: String,
) -> Result<()> {
    repository.sources.clear();
    #[cfg(windows)]
    {
        session.helper = Some(
            crate::platform::helper::start(crate::platform::helper::StartTunnel {
                core_version,
                repository,
                selected,
                proxy_port,
                metrics_port,
            })
            .await?,
        );
        session.proxy_port = proxy_port;
        session.metrics_port = metrics_port;
        session.started = Some(Instant::now());
        for _ in 0..100 {
            if session.helper.as_ref().is_some_and(|h| h.exited()) {
                return Err(BackendError::new("TUN_FAILED", "Tunnel helper exited"));
            }
            if tokio::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, proxy_port))
                .await
                .is_ok()
            {
                crate::platform::network::verify_tunnel_routes()?;
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
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
    #[cfg(windows)]
    {
        session.child = Some(crate::platform::process::CoreProcess::spawn(core, bytes).await?);
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
        let Ok(ip) = public_ip(proxy_port).await else {
            return;
        };
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
    if let Err(error) = stop(session).await {
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
fn schedule_recovery(state: Arc<AppState>, app: tauri::AppHandle, server_id: String) {
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
    let mut shutdown = state.shutdown.subscribe();
    tauri::async_runtime::spawn(async move {
        let client = match reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_millis(800))
            .build()
        {
            Ok(c) => c,
            Err(_) => return,
        };
        let mut previous = (0u16, 0u64, 0u64, Instant::now());
        let mut last_health = Instant::now();
        let mut unhealthy = 0u8;
        let mut ticker = tokio::time::interval(Duration::from_secs(1));
        loop {
            tokio::select! {
                _=shutdown.changed()=>break,
                _=ticker.tick()=>{
                    let Ok(operation)=state.connection_operation.try_acquire()else{continue};
                    let mut session=std::mem::take(&mut *state.session.lock().await);
                    let active_id = state.snapshot.read().await.info.as_ref().map(|info| info.server_id.clone());
                    let mut failed_id = None;
                    #[cfg(windows)] if session.helper.as_ref().is_some_and(|h|h.exited()) {
                        failed_id = active_id.clone();
                        let _=stop(&mut session).await;
                        *state.traffic.write().await=Default::default();
                        publish(&state,&app,Snapshot{status:"error".into(),info:None,error:Some(BackendError::new("CORE_CRASHED","Tunnel helper stopped unexpectedly"))}).await;
                    }
                    if let Some(child)=session.child.as_mut(){
                        if let Ok(Some(exit))=child.try_wait(){
                            failed_id = active_id.clone();
                            log::warn!("Xray exited with status {exit}");
                            let _=stop(&mut session).await;
                            *state.traffic.write().await=Default::default();
                            publish(&state,&app,Snapshot{status:"error".into(),info:None,error:Some(BackendError::new("CORE_CRASHED","Xray stopped unexpectedly"))}).await;
                        }
                    }
                    let active=session.started.map(|s|(session.metrics_port,s.elapsed().as_secs()));
                    *state.session.lock().await=session;
                    drop(operation);
                    if let Some(id) = failed_id {
                        schedule_recovery(state.clone(), app.clone(), id);
                    }
                    if let Some((port,duration))=active{
                        if last_health.elapsed() >= Duration::from_secs(30) {
                            last_health = Instant::now();
                            if state.repository.read().await.settings.auto_reconnect {
                                if public_ip(port).await.is_ok() { unhealthy = 0; }
                                else { unhealthy = unhealthy.saturating_add(1); }
                                if unhealthy >= 3 {
                                    let Ok(operation) = state.connection_operation.try_acquire() else { continue };
                                    let mut current = state.session.lock().await;
                                    if current.started.is_some() && current.proxy_port == port {
                                        let id = state.snapshot.read().await.info.as_ref().map(|info| info.server_id.clone());
                                        let _ = stop(&mut current).await;
                                        *state.traffic.write().await = Default::default();
                                        publish(&state, &app, Snapshot { status: "error".into(), info: None, error: Some(BackendError::new("SERVER_UNREACHABLE", "Connection health check failed")) }).await;
                                        drop(current);
                                        drop(operation);
                                        if let Some(id) = id { schedule_recovery(state.clone(), app.clone(), id); }
                                    }
                                    unhealthy = 0;
                                    continue;
                                }
                            } else { unhealthy = 0; }
                        }
                        let mut traffic=super::stats_service::TrafficStats{duration_seconds:duration,..Default::default()};
                        if let Ok((up,down))=super::stats_service::counters(&client,port).await{
                            let dt=previous.3.elapsed().as_secs_f64().max(0.001);
                            traffic.available=true;traffic.upload=up;traffic.download=down;
                            if previous.0==port{traffic.upload_speed=up.saturating_sub(previous.1) as f64/dt;traffic.download_speed=down.saturating_sub(previous.2) as f64/dt;}
                            previous=(port,up,down,Instant::now());
                        }
                        let Ok(_publish_operation)=state.connection_operation.try_acquire()else{continue};
                        let current=state.session.lock().await;
                        if current.started.is_none()||current.metrics_port!=port{continue;}
                        drop(current);
                        *state.traffic.write().await=traffic.clone();
                        if app.emit("vpn://stats",traffic).is_err(){log::warn!("Unable to deliver metrics event");}
                    }
                }
            }
        }
    });
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
}
