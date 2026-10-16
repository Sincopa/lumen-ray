use super::vpn_service::{publish, release_to_standby, schedule_recovery, stop, Snapshot};

/// A network-address change is not an outage. The tunnel is released only after
/// repeated failed checks, and that release keeps the elevated helper.
pub(crate) fn should_release_tunnel(unhealthy: u8, physical_changed: bool) -> bool {
    let _ = physical_changed;
    unhealthy >= 3
}
use crate::app::state::AppState;
use crate::domain::errors::BackendError;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tauri::Emitter;

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
        let client = match reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_millis(800))
            .build()
        {
            Ok(c) => c,
            Err(_) => return,
        };
        let mut previous = (0u64, 0u64, 0u64, Instant::now());
        let mut last_health = Instant::now();
        let mut last_tick = Instant::now();
        let mut generation = 0;
        #[cfg(windows)]
        let watcher = crate::platform::network_watch::NetworkWatch::new();
        #[cfg(windows)]
        let mut revision = watcher.revision();
        #[cfg(windows)]
        let mut physical = crate::platform::network_watch::physical_fingerprint();
        let mut physical_changed = false;
        let mut network_dirty = false;
        let mut unhealthy = 0u8;
        // The VPN server stopped answering while the tunnel itself still works.
        let mut degraded = false;
        #[cfg(windows)]
        let mut last_reassert = Instant::now() - Duration::from_secs(120);
        let mut ticker = tokio::time::interval(Duration::from_secs(1));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                _=shutdown.changed()=>break,
                _=ticker.tick()=>{
                    network_dirty |= last_tick.elapsed() > Duration::from_secs(15);
                    last_tick = Instant::now();
                    #[cfg(windows)] { let now = watcher.revision(); network_dirty |= now != revision; revision = now; }
                    let Ok(operation)=state.connection_operation.try_acquire()else{continue};
                    let mut session=std::mem::take(&mut *state.session.lock().await);
                    let active_id = state.snapshot.read().await.info.as_ref().map(|info| info.server_id.clone());
                    let mut failed_id = None;
                    if session.failed() {
                        failed_id = active_id.clone();
                        log::warn!("Core stopped unexpectedly");
                        let _=stop(&mut session).await;
                        *state.traffic.write().await=Default::default();
                        if active_id.is_some() {
                            publish(&state,&app,Snapshot{status:"error".into(),info:None,error:Some(BackendError::new("CORE_CRASHED","Networking engine stopped unexpectedly"))}).await;
                        }
                    }
                    let source = session.stats_source();
                    let active=session.started().map(|s|(session.proxy_port(),s.elapsed().as_secs(),session.generation,session.is_tun()));
                    *state.session.lock().await=session;
                    drop(operation);
                    if let Some(id) = failed_id {
                        schedule_recovery(state.clone(), app.clone(), id);
                    }
                    if let Some((proxy_port,duration,active_generation,is_tun))=active{
                        if generation != active_generation {
                            generation = active_generation; unhealthy = 0; degraded = false; network_dirty = false; physical_changed = false; last_health = Instant::now();
                            #[cfg(windows)] { physical = crate::platform::network_watch::physical_fingerprint(); }
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
                                #[cfg(windows)]
                                let offline = {
                                    let now=crate::platform::network_watch::physical_fingerprint();
                                    physical_changed |= physical.is_some() && now.is_some() && physical != now;
                                    if now.is_some() { physical=now; }
                                    now.is_none()
                                };
                                #[cfg(not(windows))]
                                let offline = false;
                                #[allow(unused_mut)]
                                let mut probe = super::health::probe(proxy_port, is_tun).await;
                                // An Xray restart or another program can leave LumenTun without the
                                // route Windows selects. Reinstall it in place instead of counting
                                // down to a disconnect.
                                #[cfg(windows)]
                                if is_tun && probe.routes.is_err() && last_reassert.elapsed() >= Duration::from_secs(60) {
                                    last_reassert = Instant::now();
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
                                        unhealthy = 0;
                                        degraded = false;
                                    }
                                    super::health::Verdict::Upstream => {
                                        if !degraded { log::warn!("VPN server is not answering; the tunnel and direct traffic stay up"); }
                                        unhealthy = 0;
                                        degraded = true;
                                    }
                                    super::health::Verdict::Routes | super::health::Verdict::Broken => {
                                        unhealthy = unhealthy.saturating_add(1);
                                    }
                                }
                                // With no physical network there is nothing to fall back to:
                                // keep the tunnel and let it resume when the network returns.
                                // Failures while offline do not count once the network is back.
                                if offline { unhealthy = 0; }
                                let failed = !offline && should_release_tunnel(unhealthy, physical_changed);
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
                                            *state.traffic.write().await = Default::default();
                                            let error = cleanup.err().or_else(|| outcome.err()).unwrap_or_else(|| BackendError::new("NETWORK_CHANGED", "Physical network changed"));
                                            log::warn!("Tunnel released after repeated failed checks: {}", error.code);
                                            publish(&state, &app, Snapshot { status: "error".into(), info: None, error: Some(error) }).await;
                                            drop(current);
                                            drop(operation);
                                            if settings.auto_reconnect { if let Some(id) = id { schedule_recovery(state.clone(), app.clone(), id); } }
                                            unhealthy = 0;
                                            physical_changed = false;
                                            continue;
                                        }
                                        log::warn!("Tunnel release failed; leaving the forwarding process in place");
                                    }
                                    continue;
                                }
                            }
                        }
                        let mut traffic=super::stats_service::TrafficStats{duration_seconds:duration,..Default::default()};
                        if let Some(source)=source { if let Ok((up,down))=source.counters(&client).await{
                            let dt=previous.3.elapsed().as_secs_f64().max(0.001);
                            traffic.available=true;traffic.upload=up;traffic.download=down;
                            if previous.0==active_generation{traffic.upload_speed=up.saturating_sub(previous.1) as f64/dt;traffic.download_speed=down.saturating_sub(previous.2) as f64/dt;}
                            previous=(active_generation,up,down,Instant::now());
                        }
                        }
                        let Ok(_publish_operation)=state.connection_operation.try_acquire()else{continue};
                        let current=state.session.lock().await;
                        if current.started().is_none()||current.generation!=active_generation{continue;}
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
mod tests {
    use super::should_release_tunnel;

    #[test]
    fn physical_change_does_not_release_and_three_failures_do() {
        assert!(!should_release_tunnel(0, true));
        assert!(!should_release_tunnel(2, true));
        assert!(!should_release_tunnel(2, false));
        assert!(should_release_tunnel(3, false));
        assert!(should_release_tunnel(4, true));
    }
}
