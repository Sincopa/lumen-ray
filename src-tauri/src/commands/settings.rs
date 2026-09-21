use crate::{
    app::state::AppState,
    domain::errors::{BackendError, Result},
    storage::repository::Settings,
};
use std::sync::Arc;
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsDto {
    #[serde(flatten)]
    pub settings: Settings,
    pub core_version: String,
}
#[tauri::command]
pub async fn get_settings(state: tauri::State<'_, Arc<AppState>>) -> Result<SettingsDto> {
    let mut settings = state.repository.read().await.settings.clone();
    #[cfg(windows)]
    {
        settings.auto_start = crate::platform::windows::is_autostart();
    }
    Ok(SettingsDto {
        settings,
        core_version: super::updates::version(&state.core.read().await),
    })
}
#[tauri::command]
pub async fn set_settings(
    state: tauri::State<'_, Arc<AppState>>,
    app: tauri::AppHandle,
    mut settings: Settings,
) -> Result<SettingsDto> {
    if !["tun", "proxy"].contains(&settings.network_mode.as_str()) {
        return Err(BackendError::new(
            "INVALID_CONFIG",
            "Unknown networking mode",
        ));
    }
    #[cfg(windows)]
    {
        let _ = crate::platform::windows::set_autostart(settings.auto_start);
        settings.auto_start = crate::platform::windows::is_autostart();
    }
    let changed;
    {
        let _guard = state.mutations.lock().await;
        let mut repo = state.repository.read().await.clone();
        changed = repo.settings.network_mode != settings.network_mode;
        repo.settings = settings.clone();
        state.store.save(&repo)?;
        *state.repository.write().await = repo;
    }
    let id = if changed {
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
        crate::services::vpn_service::connect(&state, &app, id).await?;
    }
    Ok(SettingsDto {
        settings,
        core_version: super::updates::version(&state.core.read().await),
    })
}

#[tauri::command]
pub async fn reset_network_settings(state: tauri::State<'_, Arc<AppState>>) -> Result<()> {
    #[cfg(windows)]
    {
        crate::platform::proxy::SystemProxy::recover(&state.data)?;
    }
    Ok(())
}
