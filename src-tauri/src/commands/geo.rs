use crate::{
    app::state::AppState,
    domain::errors::Result,
    xray::geo::{self, GeoDatabaseInfo},
};
use std::sync::Arc;

#[tauri::command]
pub async fn get_geo_database_info(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<GeoDatabaseInfo> {
    Ok(geo::info(&state.data))
}

#[tauri::command]
pub async fn update_geo_databases(
    state: tauri::State<'_, Arc<AppState>>,
    app: tauri::AppHandle,
) -> Result<GeoDatabaseInfo> {
    let _guard = state.updates.lock().await;
    let core = state.core.read().await.clone();
    let previous = std::fs::read(geo::metadata_path(&state.data)).ok();
    let info = geo::update(&state.data, &core).await?;
    let active = state.snapshot.read().await.info.as_ref().map(|i| i.server_id.clone());
    let apply = async {
        let repo = state.repository.read().await.clone();
        if let Some(id) = &repo.selected_server_id {
            let config = crate::xray::config::build(&repo, id, 10808, 10809, false)?;
            crate::services::vpn_service::validate(&core, &serde_json::to_vec(&config)
                .map_err(|_| crate::domain::errors::BackendError::invalid())?).await?;
        }
        if let Some(id) = &active {
            // Xray loads geo matchers when it starts; replacing a file cannot reload them.
            crate::services::vpn_service::connect_exact(&state, &app, id.clone()).await?;
        }
        Ok::<_, crate::domain::errors::BackendError>(())
    }.await;
    if let Err(error) = apply {
        if let Some(previous) = previous {
            geo::restore_metadata(&state.data, &previous)?;
            if let Some(id) = active {
                let _ = crate::services::vpn_service::connect_exact(&state, &app, id).await;
            }
        }
        return Err(error);
    }
    if active.is_none() {
        let warm_state = state.inner().clone();
        tauri::async_runtime::spawn(async move { crate::services::vpn_service::prewarm(&warm_state).await; });
    }
    Ok(info)
}
