#[cfg(target_os = "android")]
pub mod android;
#[cfg(target_os = "macos")]
#[path = "macos/autostart.rs"]
pub mod autostart;
pub mod device_identity;
#[cfg(windows)]
pub mod helper;
#[cfg(windows)]
pub mod icons;
#[cfg(windows)]
pub mod network;
#[cfg(target_os = "macos")]
#[path = "macos/network.rs"]
pub mod network;
#[cfg(windows)]
pub mod network_watch;
#[cfg(target_os = "macos")]
#[path = "macos/network_watch.rs"]
pub mod network_watch;
#[cfg(windows)]
pub mod process;
#[cfg(target_os = "macos")]
#[path = "macos/process.rs"]
pub mod process;
#[cfg(target_os = "macos")]
#[path = "macos/processes.rs"]
pub mod processes;
#[cfg(windows)]
pub mod proxy;
#[cfg(target_os = "macos")]
#[path = "macos/proxy.rs"]
pub mod proxy;
#[cfg(all(windows, debug_assertions))]
pub mod smoke;
#[cfg(windows)]
pub mod staging;
#[cfg(windows)]
pub mod windows;
