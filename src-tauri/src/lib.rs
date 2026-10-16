mod app;
mod commands;
mod domain;
mod engine;
mod platform;
mod runtime;
mod services;
mod storage;
mod subscription;
#[cfg(test)]
mod tests;
mod xray;
use commands::vpn;
use tauri::Manager;
pub fn run_startup_check_if_requested() -> bool {
    if std::env::args().nth(1).as_deref() != Some("--lumen-startup-check") {
        return false;
    }
    let context = tauri::generate_context!();
    let result = app::startup::verify_embedded_frontend(&context)
        .and_then(|_| app::startup::preflight())
        .and_then(|_| {
            runtime::resolve()
                .map(|_| ())
                .map_err(|error| app::startup::classify(&error.to_string()))
        });
    match result {
        Ok(()) => {
            app::startup::record("STARTUP_CHECK_OK");
            println!("STARTUP_CHECK_OK (embedded frontend, WebView2 and runtime discovery)");
        }
        Err(failure) => {
            app::startup::record(failure.code);
            eprintln!("{}: {}", failure.code, failure.advice);
            std::process::exit(1);
        }
    }
    true
}

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
    if matches!(
        std::env::args().nth(1).as_deref(),
        Some(
            "--lumen-smoke-tun"
                | "--lumen-smoke-warm"
                | "--lumen-smoke-warm-live"
                | "--lumen-smoke-standby"
        )
    ) {
        let warm = std::env::args().nth(1).as_deref() == Some("--lumen-smoke-warm");
        let live = std::env::args().nth(1).as_deref() == Some("--lumen-smoke-warm-live");
        let standby = std::env::args().nth(1).as_deref() == Some("--lumen-smoke-standby");
        match tokio::runtime::Runtime::new() {
            Ok(runtime) => match runtime.block_on(async {
                if warm {
                    platform::smoke::warm_route_cycle().await
                } else if live {
                    platform::smoke::warm_live().await
                } else if standby {
                    platform::smoke::warm_standby_isolation().await
                } else {
                    platform::smoke::run().await
                }
            }) {
                Ok(()) => {
                    if !warm && !live && !standby {
                        println!("NATIVE_SMOKE_OK");
                    }
                }
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
    app::startup::install_panic_handler();
    let mut context = tauri::generate_context!();
    if !cfg!(dev) {
        if let Err(failure) = app::startup::verify_embedded_frontend(&context) {
            app::startup::report(failure);
            std::process::exit(1);
        }
    }
    if let Err(failure) = app::startup::preflight() {
        app::startup::report(failure);
        std::process::exit(1);
    }
    // Exercise the shipped UI with a fresh repository, without auto-connect or prewarm.
    // This is an explicit developer check; ordinary Lumen.exe launches are unchanged.
    let smoke_data = if std::env::args().any(|arg| arg == "--lumen-ui-smoke") {
        match tempfile::tempdir() {
            Ok(directory) => Some(directory),
            Err(_) => {
                app::startup::report(app::startup::classify("STORAGE_FAILED"));
                std::process::exit(1);
            }
        }
    } else {
        None
    };
    let smoke_path = smoke_data.as_ref().map(|dir| dir.path().to_path_buf());
    let ui_smoke = smoke_path.is_some();
    if ui_smoke {
        context.config_mut().identifier = "app.lumen.desktop.ui-smoke".into();
        for window in &mut context.config_mut().app.windows {
            window.title = "Lumen — проверка запуска".into();
        }
        app::startup::record("ISOLATED_UI_SMOKE");
    }
    let application = tauri::Builder::default()
        .manage(app::startup::FrontendReadiness::default())
        .plugin(tauri_plugin_single_instance::init(|app, args, _| {
            for arg in args {
                commands::deep_link::queue_import_link(app, &arg);
            }
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_deep_link::init())
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
            app::startup::frontend_ready,
            commands::deep_link::take_import_link,
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
            commands::catalog::get_rule_icons,
            commands::catalog::ping_server,
            commands::log_console::open_log_console,
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
        .setup(move |app| {
            let resources = runtime::resolve_from(Some(&app.path().resource_dir()?))?;
            app::startup::record("RUNTIME_RESOLVED");
            let data = app.path().app_local_data_dir()?;
            let data = smoke_path.clone().unwrap_or(data);
            #[cfg(debug_assertions)]
            let data = std::env::var_os("LUMEN_TEST_DATA_DIR")
                .map(std::path::PathBuf::from)
                .unwrap_or(data);
            let state = app::state::AppState::open(data, resources)?;
            if ui_smoke {
                state
                    .repository
                    .try_write()
                    .expect("fresh smoke repository")
                    .settings
                    .close_to_tray = false;
            }
            app::startup::record("APPLICATION_DATA_READY");
            let active = crate::xray::updater::current(&state.data.join("core"));
            if active != crate::xray::bundle::VERSION {
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
            if !ui_smoke {
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
                        services::vpn_service::auto_connect(&startup, &handle, id).await;
                    } else {
                        services::vpn_service::prewarm(&startup).await;
                    }
                });
            }
            app::tray::install(app, &state)?;

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
            use tauri_plugin_deep_link::DeepLinkExt;
            if let Ok(Some(urls)) = app.deep_link().get_current() {
                for url in urls {
                    commands::deep_link::queue_import_link(app.handle(), url.as_str());
                }
            }
            let handle = app.handle().clone();
            app.deep_link().on_open_url(move |event| {
                for url in event.urls() {
                    commands::deep_link::queue_import_link(&handle, url.as_str());
                }
            });
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
        .build(context);
    let application = match application {
        Ok(app) => app,
        Err(error) => {
            let failure = match &error {
                tauri::Error::Runtime(_) => app::startup::classify("WEBVIEW_INIT_FAILED"),
                _ => app::startup::classify(&error.to_string()),
            };
            app::startup::report(failure);
            std::process::exit(1);
        }
    };
    app::startup::record("APPLICATION_INITIALIZED");
    app::startup::watch_frontend(application.handle().clone());
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
                    let _ = tokio::time::timeout(std::time::Duration::from_secs(25), async {
                        let _ = services::vpn_service::disconnect(&state, &app).await;
                        let mut session = state.session.lock().await;
                        let _ = services::vpn_service::stop(&mut session).await;
                    })
                    .await;
                    let _ = state.shutdown.send(true);
                    exiting.store(2, std::sync::atomic::Ordering::SeqCst);
                    app.exit(0);
                });
            }
        }
    });
}

