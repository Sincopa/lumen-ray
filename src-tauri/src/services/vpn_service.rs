pub use crate::engine::Session;
use crate::engine::backend;
use crate::{
    app::state::AppState,
    domain::errors::{BackendError, Result},
};
use serde::Serialize;
use std::{
    sync::{atomic::Ordering, Arc},
    time::{Duration, Instant},
};
use tauri::Emitter;

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
pub async fn publish(state: &AppState, app: &tauri::AppHandle, snapshot: Snapshot) {
    *state.snapshot.write().await = snapshot.clone();
    if app.emit("vpn://state", snapshot).is_err() {
        log::warn!("Unable to deliver connection state event");
    }
    crate::app::tray::update_tray_menu(app, state);
}
/// A user connection waits for a short catalog or settings transaction (a rule
/// toggle, a subscription being applied) instead of failing with BUSY.
async fn maintenance(state: &AppState) -> Result<tokio::sync::MutexGuard<'_, ()>> {
    tokio::time::timeout(Duration::from_secs(20), state.updates.lock())
        .await
        .map_err(|_| BackendError::new("BUSY", "Maintenance is in progress"))
}
pub async fn connect(state: &Arc<AppState>, app: &tauri::AppHandle, id: String) -> Result<()> {
    let _transaction = maintenance(state).await?;
    state.recovery_attempts.lock().await.clear();
    connect_with_selection(state, app, id, true, None, true).await
}
pub async fn connect_exact(
    state: &Arc<AppState>,
    app: &tauri::AppHandle,
    id: String,
) -> Result<()> {
    let _transaction = maintenance(state).await?;
    state.recovery_attempts.lock().await.clear();
    connect_with_selection(state, app, id, false, None, true).await
}

pub async fn connect_if_current(
    state: &Arc<AppState>,
    app: &tauri::AppHandle,
    id: String,
    expected: u64,
) -> Result<()> {
    connect_with_selection(state, app, id, false, Some(expected), false).await
}

pub async fn prewarm(state: &Arc<AppState>) {
    let Ok(_operation) = state.connection_operation.acquire().await else {
        return;
    };
    if *state.shutdown.borrow() {
        return;
    }
    let repo = state.repository.read().await.clone();
    let mut session = state.session.lock().await;
    if session.started().is_some() {
        return;
    }
    let selected = match repo.selected_server_id.as_ref() {
        Some(id) => connection_candidates(state, id).await.into_iter().next(),
        None => None,
    };
    if let Err(error) = backend()
        .prewarm(state, &repo, selected.as_deref(), &mut session.backend)
        .await
    {
        log::warn!(
            "Core preparation failed: {} ({})",
            error.code,
            error.message
        );
    }
}

#[cfg(windows)]
pub async fn apply_live_rules(
    state: &Arc<AppState>,
    selected: &str,
    before: &crate::storage::repository::Repository,
    after: &crate::storage::repository::Repository,
) -> Result<crate::engine::LiveRoutingStatus> {
    let _operation = state
        .connection_operation
        .acquire()
        .await
        .map_err(|_| BackendError::new("BUSY", "Connection manager is shutting down"))?;
    let mut session = state.session.lock().await;
    backend()
        .update_routing(state, selected, before, after, &mut session.backend)
        .await
}

async fn connect_with_selection(
    state: &Arc<AppState>,
    app: &tauri::AppHandle,
    id: String,
    best: bool,
    expected_generation: Option<u64>,
    restore_previous: bool,
) -> Result<()> {
    let generation = begin_generation(&state.connection_generation, expected_generation)?;
    let requested_at = Instant::now();
    let _operation = state
        .connection_operation
        .acquire()
        .await
        .map_err(|_| BackendError::new("BUSY", "Connection manager is shutting down"))?;
    if state.connection_generation.load(Ordering::Acquire) != generation {
        return Err(cancelled());
    }
    let previous_snapshot = if restore_previous {
        Some(state.snapshot.read().await.clone())
    } else {
        None
    };
    let previous_server = previous_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.info.as_ref())
        .map(|info| info.server_id.clone());
    let previous_repository = if previous_server.is_some() {
        let repo = state.repository.read().await;
        Some(state.session.lock().await.rollback_repository(&repo))
    } else {
        None
    };
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
    let candidates = if best {
        // Isolated probe cores can take seconds and can compete with the TUN
        // helper for Windows ports and CPU. Use cached quality and try the
        // selected server first when there is no fresh measurement.
        let mut candidates = connection_candidates(state, &id).await;
        candidates.truncate(3);
        candidates
    } else {
        vec![id]
    };
    if state.connection_generation.load(Ordering::Acquire) != generation {
        return Err(cancelled());
    }
    let mut session = std::mem::take(&mut *state.session.lock().await);
    log::info!(
        "VPN timing: candidate selection {} ms",
        requested_at.elapsed().as_millis()
    );
    let mut result = Err(BackendError::new("INVALID_SERVER", "No available server"));
    for candidate in candidates {
        if state.connection_generation.load(Ordering::Acquire) != generation {
            result = Err(cancelled());
            break;
        }
        result = connect_session(state, app, candidate.clone(), &mut session, generation).await;
        if session.is_tun()
            && result.as_ref().err().is_some_and(|error| {
                error.code == "TUN_ADAPTER_TIMEOUT" || error.code == "CORE_START_FAILED"
            })
        {
            log::warn!("Windows TUN startup failed; retrying once");
            result = connect_session(state, app, candidate.clone(), &mut session, generation).await;
        }
        if result.is_ok() {
            let mut desired = state.desired_server.write().await;
            if state.connection_generation.load(Ordering::Acquire) != generation {
                result = Err(cancelled());
                break;
            }
            *desired = Some(candidate.clone());
            drop(desired);
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
        if ![
            "CORE_START_FAILED",
            "INVALID_CONFIG",
            "UNSUPPORTED_PROTOCOL",
            "SERVER_UNREACHABLE",
            "TUN_NETWORK_UNREACHABLE",
            "CORE_CRASHED",
        ]
        .contains(&result.as_ref().err().map(|error| error.code).unwrap_or(""))
        {
            break;
        }
    }
    let mut restored = false;
    if result.is_err() && state.connection_generation.load(Ordering::Acquire) == generation {
        if let (Some(previous), Some(snapshot)) =
            (previous_server.as_deref(), previous_snapshot.as_ref())
        {
            if session.started().is_some()
                && session.backend.prepared_server_id.as_deref() == Some(previous)
            {
                publish(state, app, snapshot.clone()).await;
                restored = true;
            }
        }
    }
    if result.is_err()
        && !restored
        && state.connection_generation.load(Ordering::Acquire) == generation
    {
        if let (Some(previous), Some(repo)) = (previous_server, previous_repository) {
            if connect_session_with_repository(
                state,
                app,
                previous,
                &mut session,
                generation,
                &repo,
            )
            .await
            .is_ok()
            {
                restored = true;
                log::warn!("Connection change failed; previous server restored");
            }
        }
    }
    if let (Err(error), false) = (&result, restored) {
        if let Err(cleanup) = release(&mut session, error).await {
            result = Err(cleanup);
        }
    }
    *state.session.lock().await = session;
    if let Err(error) = &result {
        log::warn!(
            "Connection failed code={}",
            error.code
        );
        if !restored && state.connection_generation.load(Ordering::Acquire) == generation {
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
        }
    }
    log::info!(
        "VPN timing: connection result {} ms",
        requested_at.elapsed().as_millis()
    );
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
    super::server_quality::rank(choices, selected, &*state.server_quality.read().await)
}
async fn connect_session(
    state: &Arc<AppState>,
    app: &tauri::AppHandle,
    id: String,
    session: &mut Session,
    generation: u64,
) -> Result<()> {
    let repo = state.repository.read().await.clone();
    connect_session_with_repository(state, app, id, session, generation, &repo).await
}

async fn connect_session_with_repository(
    state: &Arc<AppState>,
    app: &tauri::AppHandle,
    id: String,
    session: &mut Session,
    generation: u64,
    repo: &crate::storage::repository::Repository,
) -> Result<()> {
    let session_at = Instant::now();
    let protocol = protocol_name(repo, &id)?;
    let mut result = crate::engine::connect(state, repo, &id, session).await;
    let proxy_port = session.proxy_port();
    let is_tun = session.is_tun();
    #[cfg(windows)]
    if result.is_ok() && !is_tun {
        let port = session.http_port();
        if session
            .system_proxy
            .as_ref()
            .is_none_or(|proxy| proxy.port() != port)
        {
            if let Some(previous) = session.system_proxy.take() {
                if let Err(error) = previous.restore() {
                    session.system_proxy = Some(previous);
                    result = Err(error);
                }
            }
            if result.is_ok() {
                match crate::platform::proxy::SystemProxy::enable(&state.data, port) {
                    Ok(proxy) => session.system_proxy = Some(proxy),
                    Err(error) => result = Err(error),
                }
            }
        }
    }
    log::info!(
        "Local startup completed {} ms",
        session_at.elapsed().as_millis()
    );
    // The new path is already carrying traffic. Show connected, then verify.
    // A failed check still restores the previous session or tears this one down.
    let connected_at = super::catalog_service::now();
    let connected = |ip: String| Snapshot {
        status: "connected".into(),
        info: Some(ConnectionInfo {
            server_id: id.clone(),
            ip,
            protocol: protocol.into(),
            connected_at,
            demo: false,
        }),
        error: None,
    };
    if result.is_ok() && state.connection_generation.load(Ordering::Acquire) == generation {
        publish(state, app, connected(String::new())).await;
    }
    let mut verified_ip = None;
    if result.is_ok() {
        let verification = async {
            let check = super::health::verify(proxy_port, is_tun);
            tokio::pin!(check);
            loop {
                tokio::select! {
                    result=&mut check => return result,
                    _=tokio::time::sleep(Duration::from_millis(100)) => {
                        if state.connection_generation.load(Ordering::Acquire)!=generation || *state.shutdown.borrow() { return Err(cancelled()); }
                    }
                }
            }
        };
        match verification.await {
            Ok(ip) => verified_ip = ip,
            Err(error) => result = Err(error),
        }
    }
    if state.connection_generation.load(Ordering::Acquire) != generation {
        result = Err(cancelled());
    }
    #[cfg(windows)]
    {
        let active = &mut session.backend;
        if active.pending_live_switch.is_some() {
            if result.is_ok() {
                active.pending_live_switch = None;
            } else if crate::engine::xray::rollback_live_server_switch(state, active)
                .await
                .is_ok()
            {
                return result;
            }
        }
    }
    match result {
        Ok(()) => {
            session.generation = generation;
            // Discord voice reaches Lumen only through the TUN adapter.
            if is_tun {
                super::discord_voice::schedule(state.clone(), id.clone(), proxy_port, generation);
            }
            if verified_ip.is_none() {
                let state = state.clone();
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    let Ok(ip) = public_ip(proxy_port).await else {
                        return;
                    };
                    let Ok(_operation) = state.connection_operation.acquire().await else {
                        return;
                    };
                    if state.connection_generation.load(Ordering::Acquire) != generation {
                        return;
                    }
                    let mut snapshot = state.snapshot.read().await.clone();
                    if snapshot.status != "connected" {
                        return;
                    }
                    if let Some(info) = snapshot.info.as_mut() {
                        info.ip = ip;
                        publish(&state, &app, snapshot).await;
                    }
                });
            }
            if let Some(ip) = verified_ip {
                publish(state, app, connected(ip)).await;
            }
            log::info!(
                "VPN timing: connection verified {} ms",
                session_at.elapsed().as_millis()
            );
            Ok(())
        }
        Err(error) => {
            release(session, &error).await?;
            Err(error)
        }
    }
}
fn protocol_name(repo: &crate::storage::repository::Repository, id: &str) -> Result<&'static str> {
    let node = repo
        .nodes
        .iter()
        .find(|n| n.id == id)
        .ok_or_else(BackendError::invalid)?;
    Ok(match node.protocol {
        crate::domain::node::Protocol::Socks => "SOCKS5",
        crate::domain::node::Protocol::Http => "HTTP",
        crate::domain::node::Protocol::Vless => "Vless",
        crate::domain::node::Protocol::Vmess => "VMess",
        crate::domain::node::Protocol::Trojan => "Trojan",
        crate::domain::node::Protocol::Shadowsocks => "Shadowsocks",
        crate::domain::node::Protocol::Hysteria => "Hysteria",
        crate::domain::node::Protocol::Wireguard => "WireGuard",
    })
}
/// Tear down a failed or cancelled attempt. An unreachable server says nothing
/// about the local tunnel, so a healthy prepared TUN helper is kept and the next
/// attempt needs neither a new elevated helper nor a new Wintun adapter.
pub(super) async fn release_to_standby(session: &mut Session) -> Result<()> {
    // The stopped listener must not stay the Windows proxy: browsers would lose
    // every site until the user disconnected by hand.
    #[cfg(windows)]
    if let Some(proxy) = session.system_proxy.as_ref() {
        proxy.restore()?;
        session.system_proxy = None;
    }
    backend()
        .disconnect(&mut session.backend, true)
        .await?;
    #[cfg(windows)]
    if session.is_tun() && !crate::platform::network::tunnel_defaults_absent() {
        return Err(BackendError::new(
            "TUN_FAILED",
            "Could not remove tunnel routes",
        ));
    }
    Ok(())
}
async fn release(session: &mut Session, cause: &BackendError) -> Result<()> {
    // A live tunnel helper is kept. Stopping it would ask for administrator
    // permission again and can drop the default route after the reader is gone.
    #[cfg(windows)]
    if tun_helper_alive(session) {
        return backend()
            .disconnect(&mut session.backend, true)
            .await;
    }
    if !keeps_prepared_core(cause.code) {
        return stop(session).await;
    }
    #[cfg(windows)]
    if let Some(proxy) = session.system_proxy.as_ref() {
        proxy.restore()?;
        session.system_proxy = None;
    }
    backend()
        .disconnect(&mut session.backend, true)
        .await
}
#[cfg(windows)]
fn tun_helper_alive(session: &Session) -> bool {
    session.backend.is_tun
        && session
            .backend
            .helper
            .as_ref()
            .is_some_and(|helper| !helper.exited() && helper.is_standby())
}
fn keeps_prepared_core(code: &str) -> bool {
    matches!(code, "CANCELLED" | "SERVER_UNREACHABLE" | "IP_UNAVAILABLE")
}
pub async fn public_ip(proxy_port: u16) -> Result<String> {
    super::health::resolve_ip(proxy_port).await
}
pub async fn stop(session: &mut Session) -> Result<()> {
    // Keep the listener alive if restoring proxy ownership fails.
    #[cfg(windows)]
    if let Some(proxy) = session.system_proxy.as_ref() {
        proxy.restore()?;
        session.system_proxy = None;
    }
    backend()
        .disconnect(&mut session.backend, false)
        .await
}
fn cancelled() -> BackendError {
    BackendError::new(
        "CANCELLED",
        "Connection request was superseded or cancelled",
    )
}
fn begin_generation(counter: &std::sync::atomic::AtomicU64, expected: Option<u64>) -> Result<u64> {
    match expected {
        Some(value) => counter
            .compare_exchange(value, value + 1, Ordering::AcqRel, Ordering::Acquire)
            .map(|v| v + 1)
            .map_err(|_| cancelled()),
        None => Ok(counter.fetch_add(1, Ordering::AcqRel) + 1),
    }
}
pub async fn disconnect(state: &AppState, app: &tauri::AppHandle) -> Result<()> {
    state.connection_generation.fetch_add(1, Ordering::AcqRel);
    *state.desired_server.write().await = None;
    let _operation = state
        .connection_operation
        .acquire()
        .await
        .map_err(|_| BackendError::new("BUSY", "Connection manager is shutting down"))?;
    let mut session = std::mem::take(&mut *state.session.lock().await);
    let result = disconnect_session(state, app, &mut session).await;
    *state.desired_server.write().await = None;
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
    let shutdown = if session.has_system_proxy() {
        stop(session).await
    } else {
        backend()
            .disconnect(&mut session.backend, true)
            .await
    };
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
/// Connection at application start. Windows starts login items before Wi-Fi or
/// DHCP are up; a single attempt then failed verification and the tunnel stayed
/// down until the user restarted the app. Wait for a network, then retry.
pub async fn auto_connect(state: &Arc<AppState>, app: &tauri::AppHandle, id: String) {
    #[cfg(windows)]
    {
        let deadline = Instant::now() + Duration::from_secs(90);
        let mut waited = false;
        while crate::platform::network_watch::physical_fingerprint().is_none()
            && Instant::now() < deadline
        {
            if *state.shutdown.borrow() {
                return;
            }
            waited = true;
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        if waited {
            log::info!("Automatic connection waited for the network");
            // DHCP and DNS settle a moment after the adapter reports a gateway.
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }
    let mut delay = 3u64;
    for attempt in 1..=6 {
        let error = match connect(state, app, id.clone()).await {
            Ok(()) => return,
            Err(error) => error,
        };
        log::warn!(
            "Automatic connection attempt {attempt} failed: {}",
            error.code
        );
        if !super::health::retryable(error.code) && error.code != "ROUTING_CONFLICT" {
            return;
        }
        // Stop as soon as the user connects, disconnects or picks another server.
        let ours = state.connection_generation.load(Ordering::Acquire);
        let mut shutdown = state.shutdown.subscribe();
        tokio::select! {
            _ = shutdown.changed() => return,
            _ = tokio::time::sleep(Duration::from_secs(delay)) => {}
        }
        if *state.shutdown.borrow()
            || state.connection_generation.load(Ordering::Acquire) != ours
            || state.snapshot.read().await.status == "connected"
        {
            return;
        }
        delay = (delay * 2).min(30);
    }
}
pub(super) fn schedule_recovery(state: Arc<AppState>, app: tauri::AppHandle, server_id: String) {
    if state.recovery_running.swap(true, Ordering::AcqRel) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        let mut delay = 2u64;
        let mut generation = state.connection_generation.load(Ordering::Acquire);
        for _ in 0..5 {
            if *state.shutdown.borrow()
                || state.connection_generation.load(Ordering::Acquire) != generation
                || state.desired_server.read().await.as_deref() != Some(&server_id)
                || !state.repository.read().await.settings.auto_reconnect
            {
                break;
            }
            let mut shutdown = state.shutdown.subscribe();
            let cancelled_wait = async {
                while state.connection_generation.load(Ordering::Acquire) == generation {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            };
            tokio::select! {
                _ = shutdown.changed() => break,
                _ = cancelled_wait => break,
                _ = tokio::time::sleep(Duration::from_millis(delay * 1000 + generation % 400)) => {},
            }
            if *state.shutdown.borrow()
                || state.connection_generation.load(Ordering::Acquire) != generation
                || state.desired_server.read().await.as_deref() != Some(&server_id)
                || !state.repository.read().await.settings.auto_reconnect
            {
                break;
            }
            if state.snapshot.read().await.status == "connected" {
                break;
            }
            let Ok(_transaction) = state.updates.try_lock() else {
                break;
            };
            {
                let mut attempts = state.recovery_attempts.lock().await;
                if !super::health::allow_recovery(&mut attempts, Instant::now()) {
                    log::warn!("Automatic recovery stopped after repeated failures; manual connection remains available");
                    break;
                }
            }
            match connect_with_selection(
                &state,
                &app,
                server_id.clone(),
                true,
                Some(generation),
                false,
            )
            .await
            {
                Ok(()) => break,
                Err(error) if super::health::retryable(error.code) => {}
                Err(_) => break,
            }
            generation += 1;
            delay = (delay * 2).min(60);
        }
        state.recovery_running.store(false, Ordering::Release);
    });
}
pub fn monitor(state: Arc<AppState>, app: tauri::AppHandle) {
    super::vpn_monitor::monitor(state, app);
}

#[cfg(test)]
mod generation_tests {
    use super::*;

    #[cfg(windows)]
    #[tokio::test]
    async fn best_candidates_keep_fresh_quality_order_and_default_to_selected() {
        let directory = tempfile::tempdir().unwrap();
        let runtime = crate::runtime::RuntimePaths::new(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("runtime"),
        );
        let state = AppState::open(directory.path().into(), runtime).unwrap();
        let selected = crate::subscription::uri::parse("socks5://a.example:1080", "sub").unwrap();
        let faster = crate::subscription::uri::parse("socks5://b.example:1080", "sub").unwrap();
        {
            let mut repo = state.repository.write().await;
            repo.nodes = vec![selected.clone(), faster.clone()];
            repo.settings.server_selection = "best".into();
        }
        assert_eq!(
            connection_candidates(&state, &selected.id).await[0],
            selected.id
        );
        {
            let mut quality = state.server_quality.write().await;
            quality
                .entry(selected.id.clone())
                .or_default()
                .record(Some(500));
            quality
                .entry(faster.id.clone())
                .or_default()
                .record(Some(20));
        }
        assert_eq!(
            connection_candidates(&state, &selected.id).await[0],
            faster.id
        );
    }

    #[test]
    fn disconnect_cancels_pending_recovery_and_old_transaction_rollback() {
        let generation = std::sync::atomic::AtomicU64::new(0);
        let pending = begin_generation(&generation, None).unwrap();
        generation.fetch_add(1, Ordering::AcqRel); // manual disconnect
        assert_eq!(
            begin_generation(&generation, Some(pending))
                .unwrap_err()
                .code,
            "CANCELLED"
        );
        assert_eq!(generation.load(Ordering::Acquire), 2);
        assert_eq!(begin_generation(&generation, None).unwrap(), 3);
        assert!(begin_generation(&generation, Some(2)).is_err());
    }
}
