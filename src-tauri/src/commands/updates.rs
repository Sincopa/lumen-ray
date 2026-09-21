use crate::{
    app::state::AppState,
    domain::errors::{BackendError, Result},
    xray::updater,
};
use std::sync::Arc;
use tauri_plugin_updater::UpdaterExt;
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationUpdate {
    pub configured: bool,
    pub version: Option<String>,
}
fn application_updater(app: &tauri::AppHandle) -> Result<Option<tauri_plugin_updater::Updater>> {
    let (Some(endpoint), Some(key)) = (
        option_env!("LUMEN_UPDATE_ENDPOINT"),
        option_env!("LUMEN_UPDATE_PUBLIC_KEY"),
    ) else {
        return Ok(None);
    };
    let url = url::Url::parse(endpoint)
        .map_err(|_| BackendError::new("UPDATE_FAILED", "Invalid build-time update endpoint"))?;
    if url.scheme() != "https" || key.is_empty() {
        return Err(BackendError::new(
            "UPDATE_FAILED",
            "Signed HTTPS updates are required",
        ));
    }
    app.updater_builder()
        .pubkey(key)
        .endpoints(vec![url])
        .map_err(|_| BackendError::new("UPDATE_FAILED", "Invalid update endpoint"))?
        .build()
        .map(Some)
        .map_err(|_| BackendError::new("UPDATE_FAILED", "Could not initialize signed updater"))
}
#[tauri::command]
pub async fn check_application_update(app: tauri::AppHandle) -> Result<ApplicationUpdate> {
    let Some(updater) = application_updater(&app)? else {
        return Ok(ApplicationUpdate {
            configured: false,
            version: None,
        });
    };
    let update = updater
        .check()
        .await
        .map_err(|_| BackendError::new("UPDATE_FAILED", "Could not check application updates"))?;
    Ok(ApplicationUpdate {
        configured: true,
        version: update.map(|u| u.version),
    })
}
#[tauri::command]
pub async fn install_application_update(
    state: tauri::State<'_, Arc<AppState>>,
    app: tauri::AppHandle,
) -> Result<()> {
    let _guard = state.updates.lock().await;
    let updater = application_updater(&app)?.ok_or_else(|| {
        BackendError::new(
            "UPDATE_NOT_CONFIGURED",
            "A signed release channel has not been configured",
        )
    })?;
    if let Some(update) = updater
        .check()
        .await
        .map_err(|_| BackendError::new("UPDATE_FAILED", "Could not check application updates"))?
    {
        let bytes = update.download(|_, _| {}, || {}).await.map_err(|_| {
            BackendError::new(
                "UPDATE_FAILED",
                "Update download or signature verification failed",
            )
        })?;
        crate::services::vpn_service::disconnect(&state, &app).await?;
        update
            .install(bytes)
            .map_err(|_| BackendError::new("UPDATE_FAILED", "Could not install signed update"))?;
    }
    Ok(())
}
pub fn version(path: &std::path::Path) -> String {
    path.parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or(crate::xray::core::VERSION)
        .into()
}
#[tauri::command]
pub async fn check_core_update(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Option<updater::CoreUpdate>> {
    let current = version(&state.core.read().await);
    updater::check(&current).await
}
#[tauri::command]
pub async fn install_core_update(
    state: tauri::State<'_, Arc<AppState>>,
    app: tauri::AppHandle,
) -> Result<String> {
    let _guard = state.updates.lock().await;
    let previous = state.core.read().await.clone();
    let update = updater::check(&version(&previous))
        .await?
        .ok_or_else(|| BackendError::new("UPDATE_UNAVAILABLE", "Xray is already up to date"))?;
    let candidate = updater::prepare(&update.version, &state.data.join("core")).await?;
    let repo = state.repository.read().await.clone();
    if let Some(id) = &repo.selected_server_id {
        let config = crate::xray::config::build(&repo, id, 10808, 10809, false)?;
        crate::services::vpn_service::validate(
            &candidate,
            &serde_json::to_vec(&config).map_err(|_| BackendError::invalid())?,
        )
        .await?;
    }
    let active = state
        .snapshot
        .read()
        .await
        .info
        .as_ref()
        .map(|i| i.server_id.clone());
    crate::services::vpn_service::disconnect(&state, &app).await?;
    *state.core.write().await = candidate;
    if let Some(id) = active {
        if let Err(error) = crate::services::vpn_service::connect(&state, &app, id.clone()).await {
            *state.core.write().await = previous;
            if crate::services::vpn_service::connect(&state, &app, id)
                .await
                .is_err()
            {
                log::error!("Core rollback connection failed");
            }
            return Err(error);
        }
    }
    updater::activate(&state.data.join("core"), &update.version)?;
    Ok(update.version)
}
