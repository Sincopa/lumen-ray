use tauri::Emitter;

static IMPORT_LINK: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
pub fn queue_import_link(app: &tauri::AppHandle, input: &str) {
    if input.len() > 65536 || !input.starts_with("lumen://import?") {
        return;
    }
    if let Ok(mut pending) = IMPORT_LINK.lock() {
        *pending = Some(input.to_owned());
    }
    let _ = app.emit("import://pending", ());
}
#[tauri::command]
pub fn take_import_link() -> Option<String> {
    IMPORT_LINK.lock().ok()?.take()
}
