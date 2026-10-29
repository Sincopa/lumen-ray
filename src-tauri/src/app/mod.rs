#[cfg(windows)]
pub mod instances;
pub mod startup;
pub mod state;
#[cfg(not(target_os = "android"))]
pub mod tray;
#[cfg(target_os = "android")]
#[path = "tray_android.rs"]
pub mod tray;
