use crate::{
    app::state::AppState,
    domain::errors::{BackendError, Result},
    services::catalog_service as service,
};
use serde_json::Value;
use std::sync::Arc;
use tauri::Emitter;
#[tauri::command]
pub async fn get_catalog(state: tauri::State<'_, Arc<AppState>>) -> Result<Value> {
    let latencies = state.latencies.read().await;
    Ok(service::snapshot(
        &*state.repository.read().await,
        &latencies,
    ))
}
#[tauri::command]
pub async fn mutate_catalog(
    state: tauri::State<'_, Arc<AppState>>,
    app: tauri::AppHandle,
    mutation: service::Mutation,
) -> Result<Value> {
    let reconnect = matches!(
        mutation,
        service::Mutation::SetRoutingMode { .. }
            | service::Mutation::SaveRules { .. }
            | service::Mutation::RemoveRules { .. }
            | service::Mutation::ApplyPreset { .. }
            | service::Mutation::RemovePreset { .. }
    );
    let previous = if reconnect { Some(state.repository.read().await.clone()) } else { None };
    let value = service::mutate(&state, mutation).await?;
    disconnect_removed_server(&state, &app).await?;
    crate::update_tray_menu(&app, &state);
    let id = if reconnect {
        state
            .snapshot
            .read()
            .await
            .info
            .as_ref()
            .map(|i| i.server_id.clone())
    } else {
        None
    };
    if let Some(id) = id {
        #[cfg(windows)]
        if let Some(before) = previous.as_ref() {
            let after = state.repository.read().await.clone();
            match crate::services::vpn_service::apply_live_rules(&state, &id, before, &after).await {
                Ok(true) => return Ok(value),
                Ok(false) => {}
                Err(error) => {
                    let _guard = state.mutations.lock().await;
                    if serde_json::to_value(&*state.repository.read().await).ok()
                        == serde_json::to_value(&after).ok()
                    {
                        state.store.save(before)?;
                        *state.repository.write().await = before.clone();
                        let catalog = service::snapshot(before, &*state.latencies.read().await);
                        let _ = app.emit("catalog://changed", catalog);
                        crate::update_tray_menu(&app, &state);
                    }
                    return Err(error);
                }
            }
        }
        crate::services::vpn_service::connect(&state, &app, id).await?;
    } else {
        let state = state.inner().clone();
        tauri::async_runtime::spawn(async move {
            crate::services::vpn_service::prewarm(&state).await;
        });
    }
    Ok(value)
}
#[tauri::command]
pub async fn import_subscription(
    state: tauri::State<'_, Arc<AppState>>,
    app: tauri::AppHandle,
    input: String,
    name: Option<String>,
) -> Result<service::ImportResult> {
    let result = service::import(&state, input, name, None).await?;
    crate::update_tray_menu(&app, &state);
    let warm_state = state.inner().clone();
    tauri::async_runtime::spawn(async move { crate::services::vpn_service::prewarm(&warm_state).await; });
    Ok(result)
}
#[tauri::command]
pub async fn refresh_subscription(
    state: tauri::State<'_, Arc<AppState>>,
    app: tauri::AppHandle,
    id: String,
) -> Result<service::ImportResult> {
    let (input, name) = {
        let repo = state.repository.read().await;
        (
            repo.sources
                .get(&id)
                .cloned()
                .ok_or_else(BackendError::invalid)?,
            repo.subscriptions
                .iter()
                .find(|s| s.id == id)
                .ok_or_else(BackendError::invalid)?
                .name
                .clone(),
        )
    };
    let result = service::import(&state, input, Some(name), Some(id)).await?;
    disconnect_removed_server(&state, &app).await?;
    crate::update_tray_menu(&app, &state);
    let warm_state = state.inner().clone();
    tauri::async_runtime::spawn(async move { crate::services::vpn_service::prewarm(&warm_state).await; });
    Ok(result)
}
#[tauri::command]
pub async fn get_subscription_source(
    state: tauri::State<'_, Arc<AppState>>,
    id: String,
) -> Result<String> {
    state
        .repository
        .read()
        .await
        .sources
        .get(&id)
        .cloned()
        .ok_or_else(BackendError::invalid)
}
async fn disconnect_removed_server(state: &AppState, app: &tauri::AppHandle) -> Result<()> {
    let active = state
        .snapshot
        .read()
        .await
        .info
        .as_ref()
        .map(|info| info.server_id.clone());
    if let Some(id) = active {
        if !state
            .repository
            .read()
            .await
            .nodes
            .iter()
            .any(|node| node.id == id)
        {
            crate::services::vpn_service::disconnect(state, app).await?;
        }
    }
    Ok(())
}
#[cfg(windows)]
#[tauri::command]
pub async fn get_processes() -> Result<Vec<crate::platform::windows::ProcessItem>> {
    tokio::task::spawn_blocking(crate::platform::windows::processes)
        .await
        .map_err(|_| BackendError::new("PROCESS_FAILED", "Could not enumerate processes"))?
}
#[tauri::command]
pub async fn ping_server(state: tauri::State<'_, Arc<AppState>>, id: String) -> Result<Value> {
    crate::services::latency_service::ping(&state, id).await
}
