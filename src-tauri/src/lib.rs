mod vpn;
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(vpn::VpnState::default())
        .invoke_handler(tauri::generate_handler![
            vpn::connect,
            vpn::disconnect,
            vpn::reconnect,
            vpn::get_status,
            vpn::get_connection_info
        ])
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
