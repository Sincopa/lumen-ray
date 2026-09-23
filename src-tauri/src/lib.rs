mod app;
mod commands;
mod diagnostics;
mod domain;
mod platform;
mod services;
mod storage;
mod subscription;
#[cfg(test)]
mod tests;
mod xray;
use commands::vpn;
use tauri::{Emitter, Manager};
pub fn run_helper_if_requested() -> bool {
    let args: Vec<_> = std::env::args().collect();
    #[cfg(windows)]
    if args.get(1).map(String::as_str) == Some("--lumen-signal") {
        let success = args
            .get(2)
            .and_then(|s| s.parse().ok())
            .is_some_and(platform::process::signal);
        std::process::exit(if success { 0 } else { 1 });
    }
    if args.get(1).map(String::as_str) != Some("--lumen-tunnel") {
        return false;
    }
    #[cfg(windows)]
    if let (Some(id), Some(pid)) = (args.get(2), args.get(3).and_then(|s| s.parse().ok())) {
        if let Ok(runtime) = tokio::runtime::Runtime::new() {
            let _ = runtime.block_on(platform::helper::run(id, pid));
        }
    }
    true
}
pub fn run_smoke_if_requested() -> bool {
    #[cfg(all(windows, debug_assertions))]
    if std::env::args().nth(1).as_deref() == Some("--lumen-smoke-tun") {
        match tokio::runtime::Runtime::new() {
            Ok(runtime) => match runtime.block_on(platform::smoke::run()) {
                Ok(()) => println!("NATIVE_SMOKE_OK"),
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(1);
                }
            },
            Err(_) => std::process::exit(1),
        }
        return true;
    }
    false
}
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let application = tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Info)
                .filter(|metadata| metadata.target().starts_with("app_lib"))
                .max_file_size(1024 * 1024)
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepSome(3))
                .build(),
        )
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![
            vpn::connect,
            vpn::connect_exact,
            vpn::disconnect,
            vpn::reconnect,
            vpn::get_status,
            vpn::get_connection_info,
            vpn::get_connection_snapshot,
            vpn::get_stats,
            commands::catalog::get_catalog,
            commands::catalog::mutate_catalog,
            commands::catalog::import_subscription,
            commands::catalog::refresh_subscription,
            commands::catalog::get_subscription_source,
            commands::catalog::get_processes,
            commands::catalog::ping_server,
            commands::diagnostics::get_network_diagnostics,
            commands::diagnostics::open_log_console,
            commands::notifications::send_lumen_notification,
            commands::settings::get_settings,
            commands::settings::set_settings,
            commands::settings::reset_network_settings,
            commands::updates::check_core_update,
            commands::updates::install_core_update,
            commands::updates::check_application_update,
            commands::updates::install_application_update,
            commands::geo::get_geo_database_info,
            commands::geo::update_geo_databases
        ])
        .setup(|app| {
            let resources = if cfg!(debug_assertions) {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/core")
            } else {
                app.path().resource_dir()?.join("resources/core")
            };
            let data = app.path().app_local_data_dir()?;
            #[cfg(debug_assertions)]
            let data = std::env::var_os("LUMEN_TEST_DATA_DIR")
                .map(std::path::PathBuf::from)
                .unwrap_or(data);
            let state = app::state::AppState::open(data, resources.clone())?;
            if let Err(e) = crate::xray::geo::ensure_initialized(&resources, &state.data) {
                log::warn!("Geo database initialization error: {:?}", e);
            }
            let active = crate::xray::updater::current(&state.data.join("core"));
            if active != crate::xray::core::VERSION {
                let resolved = tauri::async_runtime::block_on(async {
                    tokio::time::timeout(
                        std::time::Duration::from_secs(15),
                        crate::xray::updater::prepare(&active, &state.data.join("core")),
                    )
                    .await
                });
                if let Ok(Ok(path)) = resolved {
                    tauri::async_runtime::block_on(async {
                        *state.core.write().await = path;
                    });
                } else {
                    log::warn!("Updated core unavailable; using verified bundled fallback");
                }
            }
            services::vpn_service::monitor(state.clone(), app.handle().clone());
            let startup = state.clone();
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let id = {
                    let repo = startup.repository.read().await;
                    if repo.settings.auto_connect {
                        repo.selected_server_id.clone()
                    } else {
                        None
                    }
                };
                if let Some(id) = id {
                    if services::vpn_service::connect(&startup, &handle, id)
                        .await
                        .is_err()
                    {
                        log::warn!("Automatic connection failed");
                    }
                }
            });
            let initial_menu = create_tray_menu(app.handle(), &state)?;
            let mut tray_builder = tauri::tray::TrayIconBuilder::with_id("main-tray")
                .menu(&initial_menu)
                .tooltip("Lumen")
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| {
                    let id = event.id().as_ref().to_string();
                    let state = app
                        .state::<std::sync::Arc<app::state::AppState>>()
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

            let start_minimized = std::env::args().any(|a| a == "--minimized")
                || state
                    .repository
                    .try_read()
                    .map(|r| r.settings.start_minimized)
                    .unwrap_or(false);
            if start_minimized {
                if let Some(window) = tauri::Manager::get_webview_window(app.handle(), "main") {
                    let _ = window.hide();
                }
            }

            app.manage(state);
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let state = window
                    .app_handle()
                    .state::<std::sync::Arc<app::state::AppState>>();
                let close_to_tray = state
                    .repository
                    .try_read()
                    .map(|r| r.settings.close_to_tray)
                    .unwrap_or(true);
                if close_to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .build(tauri::generate_context!());
    let application = match application {
        Ok(app) => app,
        Err(error) => {
            eprintln!("Application initialization failed: {error}");
            std::process::exit(1);
        }
    };
    // 0 = idle, 1 = cleanup in progress, 2 = safe to exit.
    let exiting = std::sync::Arc::new(std::sync::atomic::AtomicU8::new(0));
    application.run(move |app, event| {
        if let tauri::RunEvent::ExitRequested { api, .. } = event {
            if exiting.load(std::sync::atomic::Ordering::SeqCst) != 2 {
                api.prevent_exit();
                if exiting
                    .compare_exchange(
                        0,
                        1,
                        std::sync::atomic::Ordering::SeqCst,
                        std::sync::atomic::Ordering::SeqCst,
                    )
                    .is_err()
                {
                    return;
                }
                let state = app
                    .state::<std::sync::Arc<app::state::AppState>>()
                    .inner()
                    .clone();
                let app = app.clone();
                let exiting = exiting.clone();
                tauri::async_runtime::spawn(async move {
                    if services::vpn_service::disconnect(&state, &app)
                        .await
                        .is_err()
                    {
                        log::error!("Network cleanup failed; recovery journal retained");
                        exiting.store(0, std::sync::atomic::Ordering::SeqCst);
                        return;
                    }
                    let _ = state.shutdown.send(true);
                    exiting.store(2, std::sync::atomic::Ordering::SeqCst);
                    app.exit(0);
                });
            }
        }
    });
}

pub fn create_tray_menu(
    app: &tauri::AppHandle,
    state: &app::state::AppState,
) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    use tauri::menu::*;

    let snapshot = state
        .snapshot
        .try_read()
        .map(|s| s.clone())
        .unwrap_or_default();
    let repo = state
        .repository
        .try_read()
        .map(|r| r.clone())
        .unwrap_or_default();

    let is_connected = snapshot.status == "connected";
    let is_connecting = snapshot.status == "connecting";

    let status_text = if is_connected {
        let server_name = repo
            .nodes
            .iter()
            .find(|n| Some(&n.id) == repo.selected_server_id.as_ref())
            .map(|n| n.name.as_str())
            .unwrap_or("Подключено");
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
    let current_sub_nodes: Vec<_> = repo
        .nodes
        .iter()
        .filter(|n| {
            repo.selected_subscription_id
                .as_ref()
                .is_none_or(|sid| n.subscription_id == *sid)
        })
        .collect();

    if current_sub_nodes.is_empty() {
        servers_sub = servers_sub.text("no_servers", "(Нет серверов)");
    } else {
        for node in current_sub_nodes.iter().take(25) {
            let is_sel = Some(&node.id) == repo.selected_server_id.as_ref();
            let check_item = CheckMenuItem::with_id(
                app,
                format!("srv:{}", node.id),
                &node.name,
                true,
                is_sel,
                None::<&str>,
            )?;
            servers_sub = servers_sub.item(&check_item);
        }
    }
    let servers_menu = servers_sub.build()?;

    let is_global = repo.routing_mode != "rules";
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

pub fn update_tray_menu(app: &tauri::AppHandle, state: &app::state::AppState) {
    if let Some(tray) = app.tray_by_id("main-tray") {
        if let Ok(menu) = create_tray_menu(app, state) {
            let _ = tray.set_menu(Some(menu));
        }
    }
}

async fn handle_tray_menu_event(
    app: &tauri::AppHandle,
    state: &std::sync::Arc<app::state::AppState>,
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
                let _ = services::vpn_service::disconnect(state, app).await;
            } else {
                let server_id = state.repository.read().await.selected_server_id.clone();
                if let Some(srv_id) = server_id {
                    let _ = services::vpn_service::connect(state, app, srv_id).await;
                }
            }
            update_tray_menu(app, state);
        }
        "mode:global" => {
            let _ = services::catalog_service::mutate(
                state,
                services::catalog_service::Mutation::SetRoutingMode {
                    mode: "global".into(),
                },
            )
            .await;
            update_tray_menu(app, state);
        }
        "mode:rules" => {
            let _ = services::catalog_service::mutate(
                state,
                services::catalog_service::Mutation::SetRoutingMode {
                    mode: "rules".into(),
                },
            )
            .await;
            update_tray_menu(app, state);
        }
        "quit" => {
            app.exit(0);
        }
        srv if srv.starts_with("srv:") => {
            let srv_id = &srv[4..];
            let _ = services::catalog_service::mutate(
                state,
                services::catalog_service::Mutation::SelectServer {
                    id: srv_id.to_string(),
                },
            )
            .await;
            let status = state.snapshot.read().await.status.clone();
            if status == "connected" {
                let _ = services::vpn_service::connect_exact(state, app, srv_id.to_string()).await;
            }
            update_tray_menu(app, state);
        }
        _ => {}
    }
}
