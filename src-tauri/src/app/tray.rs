//! Tray menu for the running session. The window handles navigate:settings.
use std::sync::{
    atomic::{AtomicBool, AtomicU8, Ordering},
    Arc,
};

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
            super::window::reveal(app);
        }
        "settings" => {
            if let Some(window) = super::window::reveal(app) {
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
                let _ = crate::services::vpn_service::connect_exact(state, app, srv_id.to_string())
                    .await;
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
        // A menu bar item opens its menu on click; the Windows tray icon toggles the window.
        .show_menu_on_left_click(cfg!(target_os = "macos"))
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
            if cfg!(target_os = "macos") {
                return;
            }
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
                        super::window::conceal(&window);
                    } else {
                        super::window::reveal(app);
                    }
                }
            }
        });

    let connected = state
        .snapshot
        .try_read()
        .is_ok_and(|snapshot| snapshot.status == "connected");
    LIGHT_TASKBAR.store(light_taskbar(), Ordering::Release);
    SHOWN_FACE.store(face_key(connected), Ordering::Release);
    tray_builder = tray_builder
        .icon(face(connected))
        // The menu bar draws the sad template in its own light or dark color.
        .icon_as_template(cfg!(target_os = "macos") && !connected);

    tray_builder.build(app)?;
    #[cfg(windows)]
    watch_taskbar_theme(app.handle().clone());
    Ok(())
}

/// The face shown last, so an unchanged state does not touch the icon.
static SHOWN_FACE: AtomicU8 = AtomicU8::new(u8::MAX);
/// Whether the Windows taskbar uses the light theme.
static LIGHT_TASKBAR: AtomicBool = AtomicBool::new(false);

fn face_key(connected: bool) -> u8 {
    u8::from(connected) | u8::from(LIGHT_TASKBAR.load(Ordering::Acquire)) << 1
}

/// The tray face: the round Lumen face, smiling and green while the VPN is on,
/// sad while it is off. The sad face takes the taskbar's own text color (white
/// on a dark taskbar, black on a light one); the green is deeper on a light one.
fn face(connected: bool) -> tauri::image::Image<'static> {
    #[cfg(target_os = "macos")]
    {
        if connected {
            tauri::include_image!("icons/tray/smile-macos.png")
        } else {
            tauri::include_image!("icons/tray/sad-template.png")
        }
    }
    #[cfg(not(target_os = "macos"))]
    match (connected, LIGHT_TASKBAR.load(Ordering::Acquire)) {
        (true, false) => tauri::include_image!("icons/tray/smile-dark.png"),
        (true, true) => tauri::include_image!("icons/tray/smile-light.png"),
        (false, false) => tauri::include_image!("icons/tray/sad-dark.png"),
        (false, true) => tauri::include_image!("icons/tray/sad-light.png"),
    }
}

/// Shows the face for the connection status. Called with every published
/// connection state and when the taskbar theme changes.
pub fn show_connection(app: &tauri::AppHandle, connected: bool) {
    let key = face_key(connected);
    if SHOWN_FACE.swap(key, Ordering::AcqRel) == key {
        return;
    }
    let Some(tray) = app.tray_by_id("main-tray") else {
        SHOWN_FACE.store(u8::MAX, Ordering::Release);
        return;
    };
    #[cfg(target_os = "macos")]
    let _ = tray.set_icon_as_template(!connected);
    if tray.set_icon(Some(face(connected))).is_err() {
        SHOWN_FACE.store(u8::MAX, Ordering::Release);
    }
}

#[cfg(windows)]
const PERSONALIZE: &str = r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";

/// The taskbar follows "Choose your default Windows mode", not the app mode.
/// Windows without the setting has a dark taskbar.
#[cfg(windows)]
fn light_taskbar() -> bool {
    use winreg::{enums::HKEY_CURRENT_USER, RegKey};
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(PERSONALIZE)
        .and_then(|key| key.get_value::<u32, _>("SystemUsesLightTheme"))
        .is_ok_and(|value| value != 0)
}
#[cfg(not(windows))]
fn light_taskbar() -> bool {
    false
}

/// Recolors the tray face when the user switches the Windows mode. The thread
/// sleeps in the registry change notification, so it costs nothing while idle.
#[cfg(windows)]
fn watch_taskbar_theme(app: tauri::AppHandle) {
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegNotifyChangeKeyValue, RegOpenKeyExW, HKEY, HKEY_CURRENT_USER, KEY_NOTIFY,
        REG_NOTIFY_CHANGE_LAST_SET,
    };
    let spawned = std::thread::Builder::new()
        .name("tray-theme".into())
        .spawn(move || {
            let path: Vec<u16> = PERSONALIZE.encode_utf16().chain([0]).collect();
            let mut key: HKEY = std::ptr::null_mut();
            // SAFETY: `path` is NUL-terminated and `key` receives the opened handle.
            if unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, path.as_ptr(), 0, KEY_NOTIFY, &mut key) }
                != 0
            {
                return;
            }
            loop {
                // SAFETY: `key` is open with KEY_NOTIFY; the call blocks until a value changes.
                let waited = unsafe {
                    RegNotifyChangeKeyValue(
                        key,
                        0,
                        REG_NOTIFY_CHANGE_LAST_SET,
                        std::ptr::null_mut(),
                        0,
                    )
                };
                if waited != 0 {
                    break;
                }
                let light = light_taskbar();
                if LIGHT_TASKBAR.swap(light, Ordering::AcqRel) == light {
                    continue;
                }
                let connected = app.try_state::<Arc<AppState>>().is_some_and(|state| {
                    state
                        .snapshot
                        .try_read()
                        .is_ok_and(|snapshot| snapshot.status == "connected")
                });
                show_connection(&app, connected);
            }
            // SAFETY: the handle was opened above and is not used afterwards.
            unsafe { RegCloseKey(key) };
        });
    if spawned.is_err() {
        log::warn!("Tray icon will not follow the Windows theme");
    }
}

#[cfg(all(test, not(target_os = "macos")))]
mod tests {
    use super::*;

    #[test]
    fn face_follows_connection_and_taskbar_theme() {
        // The pixel at (x, y) of the face for each state.
        let pixel = |connected: bool, light: bool, x: u32, y: u32| {
            LIGHT_TASKBAR.store(light, Ordering::Release);
            let image = face(connected);
            assert_eq!((image.width(), image.height()), (32, 32));
            let at = ((y * image.width() + x) * 4) as usize;
            image.rgba()[at..at + 4].to_vec()
        };
        // The disc above the eyes.
        let disc = |connected, light| pixel(connected, light, 16, 5);
        // Off: the taskbar's text color, white on dark and near-black on light.
        assert_eq!(disc(false, false), [0xff, 0xff, 0xff, 0xff]);
        assert_eq!(disc(false, true), [0x1f, 0x1f, 0x1f, 0xff]);
        // On: the app's success green, deeper on a light taskbar.
        assert_eq!(disc(true, false), [0x86, 0xe1, 0x6f, 0xff]);
        assert_eq!(disc(true, true), [0x4a, 0x9e, 0x3a, 0xff]);
        // No background: the corners outside the disc are transparent.
        for connected in [false, true] {
            for (x, y) in [(0, 0), (31, 0), (0, 31), (31, 31)] {
                assert_eq!(pixel(connected, false, x, y)[3], 0, "({x}, {y})");
            }
        }
        // Each state and theme has its own key, so a change always redraws.
        let keys: std::collections::HashSet<_> = [false, true]
            .into_iter()
            .flat_map(|light| {
                LIGHT_TASKBAR.store(light, Ordering::Release);
                [face_key(false), face_key(true)]
            })
            .collect();
        assert_eq!(keys.len(), 4);
        LIGHT_TASKBAR.store(false, Ordering::Release);
    }
}
