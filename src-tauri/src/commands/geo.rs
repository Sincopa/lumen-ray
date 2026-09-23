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
) -> Result<GeoDatabaseInfo> {
    let _guard = state.updates.lock().await;
    let core = state.core.read().await.clone();
    geo::update(&state.data, &core).await
}
