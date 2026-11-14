//! Android has no tray. The VPN service's persistent notification and the Quick
//! Settings tile take its place (android/plugin).
use super::state::AppState;
use std::sync::Arc;

pub fn update_tray_menu(_app: &tauri::AppHandle, _state: &AppState) {}

pub fn show_connection(_app: &tauri::AppHandle, _connected: bool) {}

pub fn install(_app: &tauri::App, _state: &Arc<AppState>) -> tauri::Result<()> {
    Ok(())
}
