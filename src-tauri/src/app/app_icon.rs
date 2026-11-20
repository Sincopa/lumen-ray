//! The application icon picked in Settings > App appearance.
//!
//! The installed files keep the default (gold) icon: the executable, the
//! installer, the app bundle and the APK. The choice changes what the running
//! app shows: the window and taskbar button on Windows and Linux, the Dock on
//! macOS, and the home screen entry on Android (one activity-alias per icon).
//! The tray keeps the Lumen face whatever the choice.
use crate::domain::errors::Result;

/// Ids shared with the UI (src/lib/services/appIcon.ts) and the Android plugin.
pub const VARIANTS: [&str; 6] = ["silver", "gold", "blue", "teal", "purple", "rainbow"];
pub const DEFAULT: &str = "gold";

pub fn is_known(icon: &str) -> bool {
    VARIANTS.contains(&icon)
}

/// The 256px artwork of `icon`.
#[cfg(not(target_os = "android"))]
fn artwork(icon: &str) -> &'static [u8] {
    match icon {
        "silver" => include_bytes!("../../icons/variants/silver.png"),
        "blue" => include_bytes!("../../icons/variants/blue.png"),
        "teal" => include_bytes!("../../icons/variants/teal.png"),
        "purple" => include_bytes!("../../icons/variants/purple.png"),
        "rainbow" => include_bytes!("../../icons/variants/rainbow.png"),
        _ => include_bytes!("../../icons/variants/gold.png"),
    }
}

/// Shows `icon` now. Errors only where the platform refuses the change.
pub async fn apply(app: &tauri::AppHandle, icon: &str) -> Result<()> {
    #[cfg(target_os = "android")]
    {
        let _ = app;
        crate::platform::android::set_launcher_icon(icon).await
    }
    #[cfg(not(target_os = "android"))]
    {
        show(app, icon);
        Ok(())
    }
}

/// Puts `icon` on the main window and its taskbar button (Windows) or the
/// Dock (macOS).
#[cfg(not(target_os = "android"))]
pub fn show(app: &tauri::AppHandle, icon: &str) {
    #[cfg(windows)]
    {
        use tauri::Manager;
        let Some(window) = app.get_webview_window("main") else {
            return;
        };
        match decode(artwork(icon)) {
            Some(image) => {
                if let Err(error) = window.set_icon(image) {
                    log::warn!("Could not set the window icon: {error}");
                }
            }
            None => log::warn!("Could not decode the {icon} icon"),
        }
    }
    #[cfg(target_os = "macos")]
    {
        let bytes = artwork(icon);
        let _ = app.run_on_main_thread(move || dock_icon(bytes));
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    let _ = (app, icon);
}

/// The 8-bit RGBA pixels of a bundled PNG (tauri icon writes RGBA).
#[cfg(windows)]
fn decode(bytes: &[u8]) -> Option<tauri::image::Image<'static>> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let mut pixels = vec![0; reader.output_buffer_size()];
    let frame = reader.next_frame(&mut pixels).ok()?;
    pixels.truncate(frame.buffer_size());
    let rgba = match frame.color_type {
        png::ColorType::Rgba => pixels,
        png::ColorType::Rgb => pixels
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|&[r, g, b]| [r, g, b, u8::MAX])
            .collect(),
        _ => return None,
    };
    Some(tauri::image::Image::new_owned(
        rgba,
        frame.width,
        frame.height,
    ))
}

#[cfg(all(test, windows))]
mod tests {
    #[test]
    fn every_icon_is_known_and_decodes() {
        assert!(super::is_known(super::DEFAULT));
        for icon in super::VARIANTS {
            let image = super::decode(super::artwork(icon)).expect("bundled icon decodes");
            assert_eq!((image.width(), image.height()), (256, 256));
            assert_eq!(image.rgba().len(), 256 * 256 * 4);
        }
        assert!(!super::is_known("star"));
    }
}

/// NSApp.applicationIconImage lasts while Lumen runs; the bundle keeps its own.
#[cfg(target_os = "macos")]
fn dock_icon(bytes: &'static [u8]) {
    use objc2::AllocAnyThread;
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::NSData;
    let Some(main) = MainThreadMarker::new() else {
        return;
    };
    let data = NSData::with_bytes(bytes);
    let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) else {
        log::warn!("Could not decode the Dock icon");
        return;
    };
    let app = NSApplication::sharedApplication(main);
    unsafe { app.setApplicationIconImage(Some(&image)) };
}
