use crate::{
    app::state::AppState,
    domain::errors::{BackendError, Result},
    xray::updater,
};
use std::sync::Arc;
#[cfg(not(target_os = "android"))]
use tauri_plugin_updater::UpdaterExt;

#[cfg(not(target_os = "android"))]
const DEFAULT_UPDATE_ENDPOINT: &str =
    "https://github.com/Sincopa/lumen-ray/releases/latest/download/latest.json";
// Public verification keys are safe to ship with the application. The private signing key
// remains stored only in GitHub Actions secrets.
#[cfg(not(target_os = "android"))]
const DEFAULT_UPDATE_PUBLIC_KEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDI4NzdGRUNENEU4QzJBMDgKUldRSUtveE96ZjUzS1BpTnFIR1ozMlNYSDNPcXNlZzFwSXJmeWVjbGh3OEhXcm8xRmMxRlJobEEK";

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationUpdate {
    pub current_version: String,
    pub version: Option<String>,
}
#[cfg(not(target_os = "android"))]
fn application_updater(app: &tauri::AppHandle) -> Result<tauri_plugin_updater::Updater> {
    let endpoint = option_env!("LUMEN_UPDATE_ENDPOINT").unwrap_or(DEFAULT_UPDATE_ENDPOINT);
    let key = option_env!("LUMEN_UPDATE_PUBLIC_KEY")
        .filter(|key| !key.trim().is_empty())
        .unwrap_or(DEFAULT_UPDATE_PUBLIC_KEY);
    let url = url::Url::parse(endpoint)
        .map_err(|_| BackendError::new("UPDATE_FAILED", "Invalid build-time update endpoint"))?;
    if url.scheme() != "https" {
        return Err(BackendError::new(
            "UPDATE_FAILED",
            "Signed HTTPS updates are required",
        ));
    }
    app.updater_builder()
        .pubkey(key)
        .endpoints(vec![url])
        .map_err(|_| BackendError::new("UPDATE_FAILED", "Invalid update endpoint"))?
        .build()
        .map_err(|_| BackendError::new("UPDATE_FAILED", "Could not initialize signed updater"))
}
#[cfg(not(target_os = "android"))]
#[tauri::command]
pub async fn check_application_update(app: tauri::AppHandle) -> Result<ApplicationUpdate> {
    let current_version = app.package_info().version.to_string();
    let updater = application_updater(&app)?;
    let update = updater
        .check()
        .await
        .map_err(|_| BackendError::new("UPDATE_FAILED", "Could not check application updates"))?;
    Ok(ApplicationUpdate {
        current_version,
        version: update.map(|u| u.version),
    })
}
#[cfg(not(target_os = "android"))]
#[tauri::command]
pub async fn install_application_update(
    state: tauri::State<'_, Arc<AppState>>,
    app: tauri::AppHandle,
) -> Result<()> {
    let _guard = state.updates.lock().await;
    let updater = application_updater(&app)?;
    if let Some(update) = updater
        .check()
        .await
        .map_err(|_| BackendError::new("UPDATE_FAILED", "Could not check application updates"))?
    {
        let bytes = update.download(|_, _| {}, || {}).await.map_err(|_| {
            BackendError::new(
                "UPDATE_FAILED",
                "Update download or signature verification failed",
            )
        })?;
        crate::services::vpn_service::disconnect(&state, &app).await?;
        update
            .install(bytes)
            .map_err(|_| BackendError::new("UPDATE_FAILED", "Could not install signed update"))?;
        // The Windows installer restarts Lumen itself; on macOS only the bundle was replaced.
        #[cfg(target_os = "macos")]
        app.request_restart();
    }
    Ok(())
}
/// Android APKs come from GitHub Releases; Android itself checks that an update
/// carries the same signature before installing it.
#[cfg(target_os = "android")]
const RELEASES_API: &str = "https://api.github.com/repos/Sincopa/lumen-ray/releases/latest";
#[cfg(target_os = "android")]
const RELEASES_PAGE: &str = "https://github.com/Sincopa/lumen-ray/releases/latest";

#[cfg(target_os = "android")]
async fn latest_release_version() -> Result<String> {
    #[derive(serde::Deserialize)]
    struct Release {
        tag_name: String,
    }
    let failed = || BackendError::new("UPDATE_FAILED", "Could not check application updates");
    let release: Release = reqwest::Client::builder()
        .user_agent(concat!("Lumen/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|_| failed())?
        .get(RELEASES_API)
        .send()
        .await
        .map_err(|_| failed())?
        .error_for_status()
        .map_err(|_| failed())?
        .json()
        .await
        .map_err(|_| failed())?;
    Ok(release.tag_name.trim_start_matches('v').to_string())
}

#[cfg(target_os = "android")]
fn newer(candidate: &str, current: &str) -> bool {
    let parse = |value: &str| -> Vec<u64> {
        value
            .split(['.', '-'])
            .map_while(|part| part.parse().ok())
            .collect()
    };
    parse(candidate) > parse(current)
}

#[cfg(target_os = "android")]
#[tauri::command]
pub async fn check_application_update(app: tauri::AppHandle) -> Result<ApplicationUpdate> {
    let current_version = app.package_info().version.to_string();
    let latest = latest_release_version().await?;
    Ok(ApplicationUpdate {
        version: newer(&latest, &current_version).then_some(latest),
        current_version,
    })
}

/// Opens the release page; the APK is installed by Android's package installer.
#[cfg(target_os = "android")]
#[tauri::command]
pub async fn install_application_update() -> Result<()> {
    crate::platform::android::open_url(RELEASES_PAGE).await
}

pub fn version(path: &std::path::Path) -> String {
    path.parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or(crate::xray::bundle::VERSION)
        .into()
}
#[tauri::command]
pub async fn check_core_update(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Option<updater::CoreUpdate>> {
    // The Android core is compiled into the APK and updates with it.
    if cfg!(target_os = "android") {
        return Ok(None);
    }
    let current = version(&state.core.read().await);
    updater::check(&current).await
}
#[tauri::command]
pub async fn install_core_update(
    state: tauri::State<'_, Arc<AppState>>,
    app: tauri::AppHandle,
) -> Result<String> {
    if cfg!(target_os = "android") {
        return Err(BackendError::new(
            "PLATFORM_UNSUPPORTED",
            "Xray updates with the Android application",
        ));
    }
    let _guard = state.updates.lock().await;
    let previous = state.core.read().await.clone();
    let update = updater::check(&version(&previous))
        .await?
        .ok_or_else(|| BackendError::new("UPDATE_UNAVAILABLE", "Xray is already up to date"))?;
    let candidate = updater::prepare(&update.version, &state.data.join("core")).await?;
    let repo = state.repository.read().await.clone();
    if let Some(id) = &repo.selected_server_id {
        let config = crate::xray::config::build(
            &repo,
            id,
            10808,
            10809,
            repo.settings.network_mode == "tun",
        )?;
        crate::engine::xray::validate(
            &candidate,
            &serde_json::to_vec(&config).map_err(|_| BackendError::invalid())?,
        )
        .await?;
    }
    let generation = state
        .connection_generation
        .load(std::sync::atomic::Ordering::Acquire)
        + 1;
    let active = state
        .snapshot
        .read()
        .await
        .info
        .as_ref()
        .map(|i| i.server_id.clone());
    crate::services::vpn_service::disconnect(&state, &app).await?;
    *state.core.write().await = candidate;
    let result = async {
        if let Some(id) = &active {
            crate::services::vpn_service::connect_if_current(&state, &app, id.clone(), generation)
                .await?;
        }
        updater::activate(&state.data.join("core"), &update.version)
    }
    .await;
    if let Err(error) = result {
        *state.core.write().await = previous;
        if error.code != "CANCELLED" {
            if let Some(id) = active {
                if crate::services::vpn_service::connect_if_current(
                    &state,
                    &app,
                    id,
                    generation + 1,
                )
                .await
                .is_err()
                {
                    return Err(BackendError::new(
                        "CORE_ROLLBACK_FAILED",
                        "The update failed and the previous connection could not be restored",
                    ));
                }
            }
        }
        return Err(error);
    }
    Ok(update.version)
}
