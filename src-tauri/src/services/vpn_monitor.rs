use super::vpn_service::{public_ip, publish, schedule_recovery, stop, Snapshot};
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
                        if active_id.is_some() {
                            publish(&state,&app,Snapshot{status:"error".into(),info:None,error:Some(BackendError::new("CORE_CRASHED","Tunnel helper stopped unexpectedly"))}).await;
                        }
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
                    let active=session.started.map(|s|(session.metrics_port,session.proxy_port,s.elapsed().as_secs()));
                    *state.session.lock().await=session;
                    drop(operation);
                    if let Some(id) = failed_id {
                        schedule_recovery(state.clone(), app.clone(), id);
                    }
                    if let Some((port,proxy_port,duration))=active{
                        if last_health.elapsed() >= Duration::from_secs(30) {
                            last_health = Instant::now();
                            if state.repository.read().await.settings.auto_reconnect {
                                if public_ip(proxy_port).await.is_ok() { unhealthy = 0; }
                                else { unhealthy = unhealthy.saturating_add(1); }
                                if unhealthy >= 3 {
                                    let Ok(operation) = state.connection_operation.try_acquire() else { continue };
                                    let mut current = state.session.lock().await;
                                    if current.started.is_some() && current.proxy_port == proxy_port {
                                        let id = state.snapshot.read().await.info.as_ref().map(|info| info.server_id.clone());
                                        if let Some(id) = &id {
                                            state.latencies.write().await.insert(id.clone(), serde_json::json!("timeout"));
                                        }
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
