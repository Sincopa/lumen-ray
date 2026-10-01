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
async fn restore_catalog_if_unchanged(
    state: &AppState,
    app: &tauri::AppHandle,
    before: &crate::storage::repository::Repository,
    after: &crate::storage::repository::Repository,
) -> Result<()> {
    let _guard = state.mutations.lock().await;
    if *state.repository.read().await == *after {
        state.store.save(before)?;
        *state.repository.write().await = before.clone();
        let catalog = service::snapshot(before, &*state.latencies.read().await);
        let _ = app.emit("catalog://changed", catalog);
        crate::update_tray_menu(app, state);
    }
    Ok(())
}
#[tauri::command]
pub async fn mutate_catalog(
    state: tauri::State<'_, Arc<AppState>>,
    app: tauri::AppHandle,
    mutation: service::Mutation,
) -> Result<Value> {
    apply_mutation(state.inner(), &app, mutation).await
}
/// Persist a catalog change and apply it to the running connection. Shared by the
/// window and the tray so both take the same live-update and rollback path.
pub async fn apply_mutation(
    state: &Arc<AppState>,
    app: &tauri::AppHandle,
    mutation: service::Mutation,
) -> Result<Value> {
    let _transaction = state.updates.lock().await;
    let generation = state
        .connection_generation
        .load(std::sync::atomic::Ordering::Acquire);
    let reconnect = matches!(
        mutation,
        service::Mutation::SetRoutingMode { .. }
            | service::Mutation::SaveRules { .. }
            | service::Mutation::RemoveRules { .. }
            | service::Mutation::ApplyPreset { .. }
            | service::Mutation::RemovePreset { .. }
    );
    let previous = if reconnect {
        Some(state.repository.read().await.clone())
    } else {
        None
    };
    let value = service::mutate(state, mutation).await?;
    disconnect_removed_server(state, app).await?;
    crate::update_tray_menu(app, state);
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
            match crate::services::vpn_service::apply_live_rules(state, &id, before, &after).await {
                Ok(true) => return Ok(value),
                Ok(false) => {
                    let still_connected = state
                        .snapshot
                        .read()
                        .await
                        .info
                        .as_ref()
                        .is_some_and(|info| info.server_id == id);
                    if !still_connected {
                        let warm_state = state.clone();
                        tauri::async_runtime::spawn(async move {
                            crate::services::vpn_service::prewarm(&warm_state).await;
                        });
                        return Ok(value);
                    }
                    // Live patching is an optimization. A full reconnect is the
                    // supported fallback when the active core has changed.
                }
                Err(error) => {
                    log::warn!("Routing change needs reconnect after {}", error.code);
                }
            }
        }
        if let Err(error) =
            crate::services::vpn_service::connect_if_current(state, app, id.clone(), generation)
                .await
        {
            if let Some(before) = previous.as_ref() {
                let after = state.repository.read().await.clone();
                restore_catalog_if_unchanged(state, app, before, &after).await?;
            }
            if error.code != "CANCELLED" && state.snapshot.read().await.status != "connected" {
                let _ = crate::services::vpn_service::connect_if_current(
                    state,
                    app,
                    id,
                    generation + 1,
                )
                .await;
            }
            return Err(error);
        }
    } else {
        let state = state.clone();
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
    let prepared = service::prepare_import(&state, input, name, None).await?;
    let _transaction = state.updates.lock().await;
    let result = service::apply_import(&state, prepared).await?;
    crate::update_tray_menu(&app, &state);
    let warm_state = state.inner().clone();
    tauri::async_runtime::spawn(async move {
        crate::services::vpn_service::prewarm(&warm_state).await;
    });
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
    // Download without the maintenance transaction: a user connection must not
    // wait for a slow subscription server.
    let prepared = service::prepare_import(&state, input, Some(name), Some(id)).await?;
    let _transaction = state.updates.lock().await;
    let generation = state
        .connection_generation
        .load(std::sync::atomic::Ordering::Acquire);
    let before = state.repository.read().await.clone();
    let active = state
        .snapshot
        .read()
        .await
        .info
        .as_ref()
        .map(|info| info.server_id.clone());
    let result = service::apply_import(&state, prepared).await?;
    let after = state.repository.read().await.clone();
    if let Some(active) = active.filter(|active| {
        let old = before.nodes.iter().find(|n| n.id == *active);
        let new = after.nodes.iter().find(|n| n.id == *active);
        new.is_some() && old != new
    }) {
        if let Err(error) = crate::services::vpn_service::connect_if_current(
            &state,
            &app,
            active.clone(),
            generation,
        )
        .await
        {
            restore_catalog_if_unchanged(&state, &app, &before, &after).await?;
            if error.code != "CANCELLED" {
                let _ = crate::services::vpn_service::connect_if_current(
                    &state,
                    &app,
                    active,
                    generation + 1,
                )
                .await;
            }
            return Err(error);
        }
    }
    disconnect_removed_server(&state, &app).await?;
    crate::update_tray_menu(&app, &state);
    let warm_state = state.inner().clone();
    tauri::async_runtime::spawn(async move {
        crate::services::vpn_service::prewarm(&warm_state).await;
    });
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
#[cfg(windows)]
#[tauri::command]
pub async fn get_rule_icons(
    names: Vec<String>,
) -> Result<std::collections::HashMap<String, String>> {
    tokio::task::spawn_blocking(move || crate::platform::icons::get_rule_icons_map(&names))
        .await
        .map_err(|_| BackendError::new("PROCESS_FAILED", "Could not resolve process icons"))
}
#[cfg(not(windows))]
#[tauri::command]
pub async fn get_rule_icons(
    _names: Vec<String>,
) -> Result<std::collections::HashMap<String, String>> {
    Ok(std::collections::HashMap::new())
}
#[tauri::command]
pub async fn ping_server(state: tauri::State<'_, Arc<AppState>>, id: String) -> Result<Value> {
    crate::services::latency_service::ping(&state, id).await
}
