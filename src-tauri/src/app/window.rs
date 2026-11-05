//! Showing and hiding the main window.
//!
//! A hidden or minimized window still had a WebView2 that rendered and kept its
//! caches, ~100+ MB across the WebView2 processes while Lumen sat in the tray.
//! On Windows the WebView is now marked invisible (rendering and animation
//! frames stop) and asked to release memory. Scripts keep running, so
//! connection notifications and scheduled subscription refreshes still work.
use tauri::Manager;

/// Bring the main window to the front.
pub fn reveal(app: &tauri::AppHandle) -> Option<tauri::WebviewWindow> {
    let window = app.get_webview_window("main")?;
    set_background(&window, false);
    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_focus();
    Some(window)
}

/// Hide the main window to the tray.
pub fn conceal(window: &tauri::WebviewWindow) {
    let _ = window.hide();
    set_background(window, true);
}

/// Follow minimize and restore, which do not pass through `reveal`/`conceal`.
pub fn follow_state(window: &tauri::Window) {
    let Some(window) = window.app_handle().get_webview_window(window.label()) else {
        return;
    };
    let hidden = !window.is_visible().unwrap_or(true) || window.is_minimized().unwrap_or(false);
    set_background(&window, hidden);
}

#[cfg(windows)]
fn set_background(window: &tauri::WebviewWindow, background: bool) {
    use std::sync::atomic::{AtomicU8, Ordering};
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2_19, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW,
        COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL,
    };
    use windows_core::Interface;
    // 0 unknown, 1 foreground, 2 background: resize events repeat the same state.
    static STATE: AtomicU8 = AtomicU8::new(0);
    let next = if background { 2 } else { 1 };
    if STATE.swap(next, Ordering::SeqCst) == next {
        return;
    }
    let _ = window.with_webview(move |webview| unsafe {
        let controller = webview.controller();
        let _ = controller.SetIsVisible(!background);
        // Runtimes before ICoreWebView2_19 only get the visibility change.
        if let Ok(core) = controller
            .CoreWebView2()
            .and_then(|core| core.cast::<ICoreWebView2_19>())
        {
            let _ = core.SetMemoryUsageTargetLevel(if background {
                COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW
            } else {
                COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL
            });
        }
    });
}

#[cfg(not(windows))]
fn set_background(_window: &tauri::WebviewWindow, _background: bool) {}
