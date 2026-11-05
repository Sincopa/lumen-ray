use super::vpn_service::{publish, release_to_standby, schedule_recovery, stop, Snapshot};

/// How long checks must keep failing before the tunnel is released. VPN servers
/// regularly stall for 10-40 seconds and Xray reconnects on its own once they
/// answer; releasing sooner turned each stall into a visible disconnect, sent
/// traffic outside the tunnel and spent the automatic recovery budget.
const RELEASE_AFTER: Duration = Duration::from_secs(45);

/// A network-address change is not an outage. The tunnel is released only after
/// repeated failed checks over `RELEASE_AFTER`, and that release keeps the
/// elevated helper.
pub(crate) fn should_release_tunnel(unhealthy: u8, failing_for: Duration) -> bool {
    unhealthy >= 3 && failing_for >= RELEASE_AFTER
}
use crate::app::state::AppState;
use crate::domain::errors::BackendError;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

/// Ask the elevated helper to reinstall the tunnel default routes. No new elevation.
#[cfg(windows)]
async fn reassert_routes(state: &AppState, generation: u64) -> bool {
    let Ok(_operation) = state.connection_operation.try_acquire() else {
        return false;
    };
    let mut session = state.session.lock().await;
    if session.started().is_none() || session.generation != generation {
        return false;
    }
    match crate::engine::xray::reassert_routes(&mut session.backend).await {
        Ok(()) => {
            log::warn!("Tunnel routes reinstalled after Windows stopped selecting LumenTun");
            true
        }
        Err(error) => {
            log::warn!("Tunnel routes could not be reinstalled: {}", error.code);
            false
        }
    }
}

pub fn monitor(state: Arc<AppState>, app: tauri::AppHandle) {
    let mut shutdown = state.shutdown.subscribe();
    tauri::async_runtime::spawn(async move {
        let mut last_health = Instant::now();
        let mut last_tick = Instant::now();
        let mut generation = 0;
        #[cfg(any(windows, target_os = "macos"))]
        let watcher = crate::platform::network_watch::NetworkWatch::new();
        #[cfg(any(windows, target_os = "macos"))]
        let mut revision = watcher.revision();
        let mut network_dirty = false;
        let mut unhealthy = 0u8;
        // When the current run of failed checks started.
        let mut failing_since: Option<Instant> = None;
        // The VPN server stopped answering while the tunnel itself still works.
        let mut degraded = false;
        // `Instant::now() - 120s` panics on Windows during the first two minutes
        // after boot (autostart), so "never reasserted" is `None`.
        #[cfg(windows)]
        let mut last_reassert: Option<Instant> = None;
        let mut ticker = tokio::time::interval(Duration::from_secs(1));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                _=shutdown.changed()=>break,
                _=ticker.tick()=>{
                    network_dirty |= last_tick.elapsed() > Duration::from_secs(15);
                    last_tick = Instant::now();
                    #[cfg(any(windows, target_os = "macos"))] { let now = watcher.revision(); network_dirty |= now != revision; revision = now; }
                    let Ok(operation)=state.connection_operation.try_acquire()else{continue};
                    let mut session=std::mem::take(&mut *state.session.lock().await);
                    let active_id = state.snapshot.read().await.info.as_ref().map(|info| info.server_id.clone());
                    let mut failed_id = None;
                    if session.failed() {
                        failed_id = active_id.clone();
                        log::warn!("Core stopped unexpectedly");
                        let _=stop(&mut session).await;
                        if active_id.is_some() {
                            publish(&state,&app,Snapshot{status:"error".into(),info:None,error:Some(BackendError::new("CORE_CRASHED","Networking engine stopped unexpectedly"))}).await;
                        }
                    }
                    // macOS proxies are per service: one enabled or created since connect has none.
                    #[cfg(target_os = "macos")]
                    if network_dirty { if let Some(proxy) = &session.system_proxy { if let Err(error) = proxy.extend() { log::warn!("New network services were not pointed at Lumen: {}", error.code); } } }
                    let active=session.started().map(|_|(session.proxy_port(),session.generation,session.is_tun()));
                    *state.session.lock().await=session;
                    drop(operation);
                    if let Some(id) = failed_id {
                        schedule_recovery(state.clone(), app.clone(), id);
                    }
                    if let Some((proxy_port,active_generation,is_tun))=active{
                        if generation != active_generation {
                            generation = active_generation; unhealthy = 0; failing_since = None; degraded = false; network_dirty = false; last_health = Instant::now();
                        }
                        // Recheck sooner after a failure; one failed probe right after a
                        // network event or resume is not an outage.
                        let interval = Duration::from_secs(if unhealthy > 0 { 5 } else if degraded { 15 } else { 30 });
                        if network_dirty || last_health.elapsed() >= interval {
                            last_health = Instant::now();
                            {
                                let (settings, rules_mode) = {
                                    let repo = state.repository.read().await;
                                    (repo.settings.clone(), repo.routing_mode == "rules")
                                };
                                #[cfg(any(windows, target_os = "macos"))]
                                let offline = crate::platform::network_watch::physical_fingerprint().is_none();
                                #[cfg(not(any(windows, target_os = "macos")))]
                                let offline = false;
                                #[allow(unused_mut)]
                                let mut probe = super::health::probe(proxy_port, is_tun).await;
                                // An Xray restart or another program can leave LumenTun without the
                                // route Windows selects. Reinstall it in place instead of counting
                                // down to a disconnect; twice within the release window.
                                #[cfg(windows)]
                                if is_tun && probe.routes.is_err() && last_reassert.is_none_or(|at| at.elapsed() >= Duration::from_secs(30)) {
                                    last_reassert = Some(Instant::now());
                                    if reassert_routes(&state, active_generation).await {
                                        probe = super::health::probe(proxy_port, is_tun).await;
                                    }
                                }
                                let verdict = super::health::classify(probe.routes.is_ok(), probe.proxy.is_ok(), probe.system.is_ok(), rules_mode);
                                let outcome: Result<(), BackendError> = match (probe.routes, probe.proxy, probe.system) {
                                    (Err(error), _, _) | (_, Err(error), _) => Err(error),
                                    (_, _, Err(_)) => Err(BackendError::new("TUN_NETWORK_UNREACHABLE", "The proxy works but the system network path does not")),
                                    _ => Ok(()),
                                };
                                match verdict {
                                    super::health::Verdict::Healthy => {
                                        if degraded { log::info!("VPN server answers again"); }
                                        if failing_since.is_some() { log::info!("Connection checks pass again; the tunnel stayed up"); }
                                        unhealthy = 0;
                                        failing_since = None;
                                        degraded = false;
                                    }
                                    super::health::Verdict::Upstream => {
                                        if !degraded { log::warn!("VPN server is not answering; the tunnel and direct traffic stay up"); }
                                        unhealthy = 0;
                                        failing_since = None;
                                        degraded = true;
                                    }
                                    super::health::Verdict::Routes | super::health::Verdict::Broken => {
                                        if failing_since.is_none() {
                                            log::warn!("Connection check failed; keeping the tunnel while the server recovers");
                                        }
                                        unhealthy = unhealthy.saturating_add(1);
                                        failing_since.get_or_insert_with(Instant::now);
                                    }
                                }
                                // With no physical network there is nothing to fall back to:
                                // keep the tunnel and let it resume when the network returns.
                                // Failures while offline do not count once the network is back.
                                if offline { unhealthy = 0; failing_since = None; }
                                let failing_for = failing_since.map_or(Duration::ZERO, |at| at.elapsed());
                                let failed = !offline && should_release_tunnel(unhealthy, failing_for);
                                network_dirty = false;
                                if failed {
                                    let Ok(operation) = state.connection_operation.try_acquire() else { continue };
                                    let mut current = state.session.lock().await;
                                    if current.started().is_some() && current.generation == active_generation {
                                        let id = state.snapshot.read().await.info.as_ref().map(|info| info.server_id.clone());
                                        let cleanup = release_to_standby(&mut current).await;
                                        #[cfg(windows)]
                                        let cleared = !current.is_tun() || crate::platform::network::tunnel_defaults_absent();
                                        #[cfg(not(windows))]
                                        let cleared = true;
                                        if cleanup.is_ok() && cleared {
                                            if let Some(id) = &id {
                                                state.latencies.write().await.insert(id.clone(), serde_json::json!("timeout"));
                                            }
                                            let error = cleanup.err().or_else(|| outcome.err()).unwrap_or_else(|| BackendError::new("NETWORK_CHANGED", "Physical network changed"));
                                            log::warn!("Tunnel released after repeated failed checks: {}", error.code);
                                            publish(&state, &app, Snapshot { status: "error".into(), info: None, error: Some(error) }).await;
                                            drop(current);
                                            drop(operation);
                                            if settings.auto_reconnect { if let Some(id) = id { schedule_recovery(state.clone(), app.clone(), id); } }
                                            unhealthy = 0;
                                            failing_since = None;
                                            continue;
                                        }
                                        log::warn!("Tunnel release failed; leaving the forwarding process in place");
                                    }
                                    continue;
                                }
                            }
                        }
                    }
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::should_release_tunnel;
    use std::time::Duration;

    #[test]
    fn short_server_stalls_keep_the_tunnel() {
        let secs = Duration::from_secs;
        // Three quick failures used to release the tunnel about 10 s in.
        assert!(!should_release_tunnel(3, secs(10)));
        assert!(!should_release_tunnel(8, secs(44)));
        // Time alone is not enough: the checks themselves must keep failing.
        assert!(!should_release_tunnel(2, secs(120)));
        assert!(should_release_tunnel(3, secs(45)));
        assert!(should_release_tunnel(9, secs(90)));
    }
}
