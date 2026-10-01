use super::vpn_service::{publish, schedule_recovery, stop, Snapshot};
use crate::app::state::AppState;
use crate::domain::errors::BackendError;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tauri::Emitter;

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
                        log::warn!("engine={:?} backend stopped unexpectedly", session.engine());
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
                            generation = active_generation; unhealthy = 0; network_dirty = false; physical_changed = false; last_health = Instant::now();
                            #[cfg(windows)] { physical = crate::platform::network_watch::physical_fingerprint(); }
                        }
                        // Recheck sooner after a failure; one failed probe right after a
                        // network event or resume is not an outage.
                        let interval = Duration::from_secs(if unhealthy > 0 { 5 } else { 30 });
                        if network_dirty || last_health.elapsed() >= interval {
                            last_health = Instant::now();
                            {
                                let settings = state.repository.read().await.settings.clone();
                                #[cfg(windows)] {
                                    let now=crate::platform::network_watch::physical_fingerprint();
                                    physical_changed |= physical.is_some() && now.is_some() && physical != now;
                                    if now.is_some() { physical=now; }
                                }
                                let outcome = super::health::verify(proxy_port, is_tun).await;
                                if outcome.is_ok() { unhealthy = 0; }
                                else { unhealthy = unhealthy.saturating_add(1); }
                                let failed = unhealthy >= 3 || (physical_changed && settings.auto_reconnect);
                                network_dirty = false;
                                if failed {
                                    let Ok(operation) = state.connection_operation.try_acquire() else { continue };
                                    let mut current = state.session.lock().await;
                                    if current.started().is_some() && current.generation == active_generation {
                                        let id = state.snapshot.read().await.info.as_ref().map(|info| info.server_id.clone());
                                        if let Some(id) = &id {
                                            state.latencies.write().await.insert(id.clone(), serde_json::json!("timeout"));
                                        }
                                        let cleanup = stop(&mut current).await;
                                        *state.traffic.write().await = Default::default();
                                        let error=cleanup.err().or_else(|| outcome.err()).unwrap_or_else(|| BackendError::new("NETWORK_CHANGED", "Physical network changed"));
                                        publish(&state, &app, Snapshot { status: "error".into(), info: None, error: Some(error) }).await;
                                        drop(current);
                                        drop(operation);
                                        if settings.auto_reconnect { if let Some(id) = id { schedule_recovery(state.clone(), app.clone(), id); } }
                                    }
                                    unhealthy = 0;
                                    physical_changed = false;
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
