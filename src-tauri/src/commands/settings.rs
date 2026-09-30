use crate::{
    app::state::AppState,
    domain::errors::{BackendError, Result},
    storage::repository::Settings,
};
use std::sync::Arc;
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsDto {
    #[serde(flatten)]
    pub settings: Settings,
    pub core_version: String,
    pub core_engines: Vec<crate::core::CoreCapabilities>,
}
#[tauri::command]
pub async fn get_settings(state: tauri::State<'_, Arc<AppState>>) -> Result<SettingsDto> {
    let mut settings = state.repository.read().await.settings.clone();
    #[cfg(windows)]
    {
        settings.auto_start = crate::platform::windows::is_autostart();
    }
    Ok(SettingsDto {
        settings,
        core_engines: [
            crate::core::CoreEngine::Lumen,
            crate::core::CoreEngine::Xray,
        ]
        .into_iter()
        .map(|engine| crate::core::backend(engine).capabilities())
        .collect(),
        core_version: super::updates::version(&state.core.read().await),
    })
}
#[tauri::command]
pub async fn set_settings(
    state: tauri::State<'_, Arc<AppState>>,
    app: tauri::AppHandle,
    mut settings: Settings,
) -> Result<SettingsDto> {
    let _transaction = state.updates.lock().await;
    validate_settings(&settings)?;
    #[cfg(windows)]
    {
        if settings.auto_start != crate::platform::windows::is_autostart() {
            crate::platform::windows::set_autostart(settings.auto_start)?;
        }
        settings.auto_start = crate::platform::windows::is_autostart();
    }
    let changed;
    let previous;
    {
        let _guard = state.mutations.lock().await;
        let mut repo = state.repository.read().await.clone();
        previous = repo.settings.clone();
        changed = repo.settings.network_mode != settings.network_mode
            || repo.settings.dns_policy != settings.dns_policy
            || repo.settings.dns_server != settings.dns_server
            || repo.settings.bypass_lan != settings.bypass_lan;
        repo.settings = settings.clone();
        state.store.save(&repo)?;
        *state.repository.write().await = repo;
    }
    let generation = state
        .connection_generation
        .load(std::sync::atomic::Ordering::Acquire);
    let id = if changed {
        state
            .snapshot
            .read()
            .await
            .info
            .as_ref()
            .map(|i| i.server_id.clone())
    } else {
        None
    };
    if let Some(id) = id {
        if let Err(error) =
            crate::services::vpn_service::connect_if_current(&state, &app, id.clone(), generation)
                .await
        {
            let _guard = state.mutations.lock().await;
            let mut repo = state.repository.read().await.clone();
            repo.settings.network_mode = previous.network_mode;
            repo.settings.dns_server = previous.dns_server;
            repo.settings.dns_policy = previous.dns_policy;
            repo.settings.bypass_lan = previous.bypass_lan;
            state.store.save(&repo)?;
            *state.repository.write().await = repo;
            drop(_guard);
            if error.code != "CANCELLED" && state.snapshot.read().await.status != "connected" {
                let _ = crate::services::vpn_service::connect_if_current(
                    &state,
                    &app,
                    id,
                    generation + 1,
                )
                .await;
            }
            return Err(error);
        }
    } else {
        let warm_state = state.inner().clone();
        tauri::async_runtime::spawn(async move {
            crate::services::vpn_service::prewarm(&warm_state).await;
        });
    }
    Ok(SettingsDto {
        settings,
        core_engines: [
            crate::core::CoreEngine::Lumen,
            crate::core::CoreEngine::Xray,
        ]
        .into_iter()
        .map(|engine| crate::core::backend(engine).capabilities())
        .collect(),
        core_version: super::updates::version(&state.core.read().await),
    })
}

#[tauri::command]
pub async fn reset_network_settings(state: tauri::State<'_, Arc<AppState>>) -> Result<()> {
    #[cfg(windows)]
    {
        crate::platform::proxy::SystemProxy::recover(&state.data)?;
    }
    Ok(())
}

pub(crate) fn validate_settings(settings: &Settings) -> Result<()> {
    settings.dns_policy.validate()?;
    if !["direct", "proxy", "auto"].contains(&settings.subscription_transport.as_str()) {
        return Err(BackendError::new(
            "INVALID_CONFIG",
            "Invalid subscription transport",
        ));
    }
    if !["tun", "proxy"].contains(&settings.network_mode.as_str()) {
        return Err(BackendError::new(
            "INVALID_CONFIG",
            "Unknown networking mode",
        ));
    }
    let dns = settings.dns_server.parse::<std::net::IpAddr>();
    let dns_valid = settings.dns_server == "system"
        || dns.is_ok_and(|address| match address {
            std::net::IpAddr::V4(ip) => {
                !ip.is_unspecified() && !ip.is_multicast() && !ip.is_broadcast()
            }
            std::net::IpAddr::V6(ip) => !ip.is_unspecified() && !ip.is_multicast(),
        });
    if !["manual", "best"].contains(&settings.server_selection.as_str())
        || !["startup", "list", "manual"].contains(&settings.ping_policy.as_str())
        || ![0, 1, 6, 24].contains(&settings.update_interval_hours)
        || !dns_valid
    {
        return Err(BackendError::new(
            "INVALID_CONFIG",
            "Invalid network setting",
        ));
    }
    let ping = url::Url::parse(&settings.ping_url)
        .map_err(|_| BackendError::new("INVALID_CONFIG", "Invalid ping URL"))?;
    if !["http", "https"].contains(&ping.scheme()) || ping.host_str().is_none() {
        return Err(BackendError::new(
            "INVALID_CONFIG",
            "Ping URL must use HTTP or HTTPS",
        ));
    }
    Ok(())
}
