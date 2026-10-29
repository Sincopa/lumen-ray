//! Android system settings a VPN client needs: Always-on VPN with "Block
//! connections without VPN" (the kill switch) and battery optimization.
//! Desktop has neither, so these report that.
#[cfg(not(target_os = "android"))]
use crate::domain::errors::BackendError;
use crate::domain::errors::Result;

#[cfg(not(target_os = "android"))]
fn unsupported() -> BackendError {
    BackendError::new("PLATFORM_UNSUPPORTED", "Available on Android only")
}

#[tauri::command]
pub async fn open_vpn_settings() -> Result<()> {
    #[cfg(target_os = "android")]
    {
        crate::platform::android::open_vpn_settings().await
    }
    #[cfg(not(target_os = "android"))]
    {
        Err(unsupported())
    }
}

/// `true` when Android will not suspend Lumen in Doze.
#[tauri::command]
pub async fn get_battery_optimization() -> Result<bool> {
    #[cfg(target_os = "android")]
    {
        crate::platform::android::battery_optimization_ignored().await
    }
    #[cfg(not(target_os = "android"))]
    {
        Err(unsupported())
    }
}

#[tauri::command]
pub async fn request_battery_exemption() -> Result<()> {
    #[cfg(target_os = "android")]
    {
        crate::platform::android::request_battery_exemption().await
    }
    #[cfg(not(target_os = "android"))]
    {
        Err(unsupported())
    }
}

/// Android back on Home: leave Lumen like Home does, with the VPN still running.
#[tauri::command]
pub async fn send_to_background() -> Result<()> {
    #[cfg(target_os = "android")]
    {
        crate::platform::android::move_to_background().await
    }
    #[cfg(not(target_os = "android"))]
    {
        Err(unsupported())
    }
}
