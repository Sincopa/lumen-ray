//! Tray menu for the running session. The window handles navigate:settings.
use std::sync::Arc;

use super::state::AppState;
use tauri::{Emitter, Manager};

fn create_tray_menu(
    app: &tauri::AppHandle,
    state: &super::state::AppState,
) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    use tauri::menu::*;

    let (is_connected, is_connecting) = state
        .snapshot
        .try_read()
        .map(|s| (s.status == "connected", s.status == "connecting"))
        .unwrap_or((false, false));

    let (selected_server_name, current_sub_nodes, is_global) = state
        .repository
        .try_read()
        .map(|r| {
            let selected_name = if is_connected {
                r.nodes
                    .iter()
                    .find(|n| Some(&n.id) == r.selected_server_id.as_ref())
                    .map(|n| n.name.clone())
            } else {
                None
            };
            let nodes: Vec<(String, String, bool)> = r
                .nodes
                .iter()
                .filter(|n| {
                    r.selected_subscription_id
                        .as_ref()
                        .is_none_or(|sid| n.subscription_id == *sid)
                })
                .take(25)
                .map(|n| {
                    (
                        n.id.clone(),
                        n.name.clone(),
                        Some(&n.id) == r.selected_server_id.as_ref(),
                    )
                })
                .collect();
            (selected_name, nodes, r.routing_mode != "rules")
        })
        .unwrap_or_else(|_| (None, Vec::new(), true));

    let status_text = if is_connected {
        let server_name = selected_server_name.as_deref().unwrap_or("Подключено");
        format!("● {}", server_name)
    } else if is_connecting {
        "◌ Подключение...".to_string()
    } else {
        "○ Отключено".to_string()
    };
    let status_item = MenuItem::with_id(app, "status_header", status_text, false, None::<&str>)?;

    let toggle_text = if is_connected {
        "Отключиться"
    } else {
        "Подключиться"
    };
    let toggle_item = MenuItem::with_id(
        app,
        "toggle_connect",
        toggle_text,
        !is_connecting,
        None::<&str>,
    )?;

    let sep1 = PredefinedMenuItem::separator(app)?;

    let mut servers_sub = SubmenuBuilder::new(app, "Серверы");

    if current_sub_nodes.is_empty() {
        servers_sub = servers_sub.text("no_servers", "(Нет серверов)");
    } else {
        for (node_id, node_name, is_sel) in &current_sub_nodes {
            let check_item = CheckMenuItem::with_id(
                app,
                format!("srv:{node_id}"),
                node_name,
                true,
                *is_sel,
                None::<&str>,
            )?;
            servers_sub = servers_sub.item(&check_item);
        }
    }
    let servers_menu = servers_sub.build()?;
    let mode_global = CheckMenuItem::with_id(
        app,
        "mode:global",
        "Все сайты (Global)",
        true,
        is_global,
        None::<&str>,
    )?;
    let mode_rules = CheckMenuItem::with_id(
        app,
        "mode:rules",
        "По правилам (Rules)",
        true,
        !is_global,
        None::<&str>,
    )?;
    let mode_menu = SubmenuBuilder::new(app, "Маршрутизация")
        .item(&mode_global)
        .item(&mode_rules)
        .build()?;

    let sep2 = PredefinedMenuItem::separator(app)?;
    let show_item = MenuItem::with_id(app, "show", "Открыть Lumen", true, None::<&str>)?;
    let settings_item = MenuItem::with_id(app, "settings", "Настройки", true, None::<&str>)?;
    let sep3 = PredefinedMenuItem::separator(app)?;
    let quit_item = MenuItem::with_id(app, "quit", "Выход", true, None::<&str>)?;

    let menu = MenuBuilder::new(app)
        .item(&status_item)
        .item(&toggle_item)
        .item(&sep1)
        .item(&servers_menu)
        .item(&mode_menu)
        .item(&sep2)
        .item(&show_item)
        .item(&settings_item)
        .item(&sep3)
        .item(&quit_item)
        .build()?;

    Ok(menu)
}

pub fn update_tray_menu(app: &tauri::AppHandle, state: &super::state::AppState) {
    if let Some(tray) = app.tray_by_id("main-tray") {
        if let Ok(menu) = create_tray_menu(app, state) {
            let _ = tray.set_menu(Some(menu));
        }
    }
}

/// Tray actions have no command result; publish the catalog the window would get.
async fn emit_catalog(
    app: &tauri::AppHandle,
    state: &super::state::AppState,
    result: crate::domain::errors::Result<serde_json::Value>,
) {
    let catalog = match result {
        Ok(catalog) => catalog,
        // A failed change may have been rolled back: show what is persisted.
        Err(_) => crate::services::catalog_service::snapshot(
            &*state.repository.read().await,
            &*state.latencies.read().await,
        ),
    };
    let _ = tauri::Emitter::emit(app, "catalog://changed", catalog);
}

async fn handle_tray_menu_event(
    app: &tauri::AppHandle,
    state: &std::sync::Arc<super::state::AppState>,
    id: &str,
) {
    match id {
        "show" => {
            if let Some(window) = tauri::Manager::get_webview_window(app, "main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }
        "settings" => {
            if let Some(window) = tauri::Manager::get_webview_window(app, "main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
                let _ = window.emit("navigate:settings", ());
            }
        }
        "toggle_connect" => {
            let status = state.snapshot.read().await.status.clone();
            if status == "connected" {
                let _ = crate::services::vpn_service::disconnect(state, app).await;
                crate::services::vpn_service::prewarm(state).await;
            } else {
                let server_id = state.repository.read().await.selected_server_id.clone();
                if let Some(srv_id) = server_id {
                    let _ = crate::services::vpn_service::connect(state, app, srv_id).await;
                }
            }
            update_tray_menu(app, state);
        }
        "mode:global" | "mode:rules" => {
            // Same path as the window: apply to the live connection and notify the UI.
            let mode = id.trim_start_matches("mode:").to_owned();
            let result = crate::commands::catalog::apply_mutation(
                state,
                app,
                crate::services::catalog_service::Mutation::SetRoutingMode { mode },
            )
            .await;
            emit_catalog(app, state, result).await;
            update_tray_menu(app, state);
        }
        "quit" => {
            app.exit(0);
        }
        srv if srv.starts_with("srv:") => {
            let srv_id = &srv[4..];
            let status = state.snapshot.read().await.status.clone();
            if status == "connected" {
                // A successful switch persists the selection and notifies the UI.
                let _ = crate::services::vpn_service::connect_exact(state, app, srv_id.to_string()).await;
            } else {
                let result = crate::commands::catalog::apply_mutation(
                    state,
                    app,
                    crate::services::catalog_service::Mutation::SelectServer {
                        id: srv_id.to_string(),
                    },
                )
                .await;
                emit_catalog(app, state, result).await;
            }
            update_tray_menu(app, state);
        }
        _ => {}
    }
}

pub fn install(app: &tauri::App, state: &Arc<AppState>) -> tauri::Result<()> {
    let initial_menu = create_tray_menu(app.handle(), state)?;
    let mut tray_builder = tauri::tray::TrayIconBuilder::with_id("main-tray")
        .menu(&initial_menu)
        .tooltip("Lumen")
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            let id = event.id().as_ref().to_string();
            let state = app
                .state::<std::sync::Arc<super::state::AppState>>()
                .inner()
                .clone();
            let app_handle = app.clone();
            tauri::async_runtime::spawn(async move {
                handle_tray_menu_event(&app_handle, &state, &id).await;
            });
        })
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::Click {
                button: tauri::tray::MouseButton::Left,
                button_state: tauri::tray::MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = tauri::Manager::get_webview_window(app, "main") {
                    let is_visible = window.is_visible().unwrap_or(false);
                    let is_minimized = window.is_minimized().unwrap_or(false);
                    if is_visible && !is_minimized {
                        let _ = window.hide();
                    } else {
                        let _ = window.show();
                        let _ = window.unminimize();
                        let _ = window.set_focus();
                    }
                }
            }
        });

    if let Some(icon) = app.default_window_icon() {
        tray_builder = tray_builder.icon(icon.clone());
    }

    tray_builder.build(app)?;
    Ok(())
}
