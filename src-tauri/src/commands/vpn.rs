use crate::{
    app::state::AppState,
    domain::errors::{BackendError, Result},
    services::vpn_service as service,
};
use std::sync::Arc;
#[tauri::command]
pub async fn connect(
    state: tauri::State<'_, Arc<AppState>>,
    app: tauri::AppHandle,
    server_id: String,
) -> Result<()> {
    service::connect(&state, &app, server_id).await
}
#[tauri::command]
pub async fn connect_exact(
    state: tauri::State<'_, Arc<AppState>>,
    app: tauri::AppHandle,
    server_id: String,
) -> Result<()> {
    service::connect_exact(&state, &app, server_id).await
}
#[tauri::command]
pub async fn disconnect(
    state: tauri::State<'_, Arc<AppState>>,
    app: tauri::AppHandle,
) -> Result<()> {
    service::disconnect(&state, &app).await
}
#[tauri::command]
pub async fn reconnect(
    state: tauri::State<'_, Arc<AppState>>,
    app: tauri::AppHandle,
) -> Result<()> {
    let id = state
        .snapshot
        .read()
        .await
        .info
        .as_ref()
        .map(|i| i.server_id.clone())
        .ok_or_else(|| BackendError::new("INVALID_SERVER", "No active server"))?;
    service::connect(&state, &app, id).await
}
#[tauri::command]
pub async fn get_status(state: tauri::State<'_, Arc<AppState>>) -> Result<String> {
    Ok(state.snapshot.read().await.status.clone())
}
#[tauri::command]
pub async fn get_connection_info(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Option<service::ConnectionInfo>> {
    Ok(state.snapshot.read().await.info.clone())
}
#[tauri::command]
pub async fn get_connection_snapshot(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<service::Snapshot> {
    Ok(state.snapshot.read().await.clone())
}
#[tauri::command]
pub async fn get_stats(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<crate::services::stats_service::TrafficStats> {
    Ok(state.traffic.read().await.clone())
}
