use crate::{
    app::state::AppState,
    domain::errors::{BackendError, Result},
};
use std::sync::Arc;
use tauri_plugin_notification::NotificationExt;

#[tauri::command]
pub async fn send_lumen_notification(
    state: tauri::State<'_, Arc<AppState>>,
    app: tauri::AppHandle,
    kind: String,
    language: String,
) -> Result<()> {
    let settings = state.repository.read().await.settings.clone();
    let english = language == "en";
    let (enabled, title, body) = match (kind.as_str(), english) {
        ("disconnect", true) => (
            settings.notify_disconnect,
            "Lumen",
            "VPN connection was interrupted",
        ),
        ("disconnect", false) => (
            settings.notify_disconnect,
            "Lumen",
            "Соединение VPN прервано",
        ),
        ("reconnect", true) => (
            settings.notify_reconnect,
            "Lumen",
            "VPN connection restored",
        ),
        ("reconnect", false) => (
            settings.notify_reconnect,
            "Lumen",
            "Соединение VPN восстановлено",
        ),
        ("expiry", true) => (
            settings.notify_expiry,
            "Lumen",
            "A subscription has expired or expires soon",
        ),
        ("expiry", false) => (
            settings.notify_expiry,
            "Lumen",
            "Подписка истекла или скоро истечёт",
        ),
        ("update", true) => (
            settings.notify_update,
            "Lumen",
            "An application update is available",
        ),
        ("update", false) => (
            settings.notify_update,
            "Lumen",
            "Доступно обновление приложения",
        ),
        _ => return Ok(()),
    };
    if enabled {
        app.notification()
            .builder()
            .title(title)
            .body(body)
            .show()
            .map_err(|_| BackendError::new("NOTIFICATION_FAILED", "Could not show notification"))?;
    }
    Ok(())
}
