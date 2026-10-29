//! Android: the core drives `LumenVpnService` through `tauri-plugin-lumen`
//! (android/plugin). Xray runs inside that service with the VpnService TUN
//! descriptor; this side builds the configuration and the per-app lists.
use crate::{
    app::state::AppState,
    domain::errors::{BackendError, Result},
    storage::repository::{Repository, RuleMatcher},
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, OnceLock,
};
use std::time::{Duration, Instant};
use tauri_plugin_lumen::{Lumen, LumenExt, StartRequest};

static BRIDGE: OnceLock<Lumen<tauri::Wry>> = OnceLock::new();
/// The service reported a failure (not a stop). `failed()` is synchronous and
/// runs inside the async runtime, so it reads this instead of calling into Kotlin.
static CRASHED: AtomicBool = AtomicBool::new(false);
static EXPECTED: AtomicBool = AtomicBool::new(false);
const WATCH_INTERVAL: Duration = Duration::from_millis(1500);
/// A tunnel that could not be adopted is retried after this long.
const ADOPT_RETRY: Duration = Duration::from_secs(60);

pub fn install(app: &tauri::AppHandle) {
    let _ = BRIDGE.set(app.lumen().clone());
}

/// Keeps this session in step with LumenVpnService, which also answers the
/// notification, the Quick Settings tile, Always-on VPN and boot:
/// - a tunnel running without this session (started while the UI was closed,
///   or this process restarted under the sticky service) is adopted, so the
///   screen shows it connected and health checks cover it;
/// - a tunnel stopped outside the UI, or taken over by another VPN app, is
///   a disconnect, not a crash to recover from.
pub fn watch(state: Arc<AppState>, app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut last_adopt: Option<Instant> = None;
        loop {
            tokio::time::sleep(WATCH_INTERVAL).await;
            if *state.shutdown.borrow() {
                break;
            }
            let Ok(bridge) = bridge() else { continue };
            let Ok(status) = bridge.status().await else {
                continue;
            };
            let revoked = status.error_code.as_deref() == Some("VPN_REVOKED");
            let stopped = status.state == "idle" || (status.state == "error" && revoked);
            CRASHED.store(status.state == "error" && !revoked, Ordering::Release);
            // Connect and disconnect hold the operation; their own transitions are not news.
            let Ok(operation) = state.connection_operation.try_acquire() else {
                continue;
            };
            drop(operation);
            if EXPECTED.load(Ordering::Acquire) && stopped {
                log::info!("Android VPN was stopped outside Lumen");
                EXPECTED.store(false, Ordering::Release);
                let _ = crate::services::vpn_service::disconnect(&state, &app).await;
            } else if !EXPECTED.load(Ordering::Acquire)
                && status.running
                && last_adopt.is_none_or(|at| at.elapsed() >= ADOPT_RETRY)
            {
                last_adopt = Some(Instant::now());
                let selected = state.repository.read().await.selected_server_id.clone();
                if let Some(id) = selected {
                    log::info!("Adopting the running Android VPN");
                    crate::services::vpn_service::auto_connect(&state, &app, id).await;
                }
            }
        }
    });
}

fn bridge() -> Result<&'static Lumen<tauri::Wry>> {
    BRIDGE.get().ok_or_else(|| {
        BackendError::new("CORE_START_FAILED", "The Android VPN bridge is not ready")
    })
}

/// Kotlin reports the desktop error codes; anything else becomes `TUN_FAILED`.
fn convert(error: tauri_plugin_lumen::Error) -> BackendError {
    const CODES: &[&str] = &[
        "VPN_PERMISSION_DENIED",
        "VPN_REVOKED",
        "NO_CONNECTION_PLAN",
        "CORE_START_FAILED",
        "INVALID_CONFIG",
        "STORAGE_FAILED",
        "PLATFORM_UNSUPPORTED",
        "APPS_UNAVAILABLE",
        "TUN_FAILED",
    ];
    let code = CODES
        .iter()
        .copied()
        .find(|code| *code == error.code)
        .unwrap_or("TUN_FAILED");
    log::warn!("Android bridge refused: {}", error.code);
    BackendError::new(code, error.message)
}

/// Packages that bypass the VPN: enabled application rules that send an app
/// direct in Rules mode. Other application targets need per-app routing inside
/// Xray, which Android does not offer.
pub fn excluded_apps(repo: &Repository) -> Vec<String> {
    if repo.routing_mode != "rules" {
        return Vec::new();
    }
    let mut apps: Vec<String> = repo
        .rules
        .iter()
        .filter(|rule| {
            rule.enabled && rule.matcher == RuleMatcher::Application && rule.server_id == "direct"
        })
        .flat_map(|rule| rule.name.split(','))
        .map(|name| name.trim().to_string())
        .filter(|name| is_package_name(name))
        .collect();
    apps.sort();
    apps.dedup();
    apps
}

fn is_package_name(name: &str) -> bool {
    name.contains('.')
        && name.len() <= 255
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_')
}

pub async fn start(
    repo: &Repository,
    selected: &str,
    config: &[u8],
    asset_dir: &str,
) -> Result<()> {
    let bridge = bridge()?;
    if !bridge.prepare().await.map_err(convert)? {
        return Err(BackendError::new(
            "VPN_PERMISSION_DENIED",
            "Allow Lumen to create a VPN connection",
        ));
    }
    let server_name = repo
        .nodes
        .iter()
        .find(|node| node.id == selected)
        .map(|node| node.name.clone())
        .unwrap_or_default();
    let request = StartRequest {
        config: String::from_utf8(config.to_vec()).map_err(|_| BackendError::invalid())?,
        asset_dir: asset_dir.into(),
        excluded_apps: excluded_apps(repo),
        included_apps: Vec::new(),
        server_name,
        // The VPN routes every address, so port 53 to it reaches Xray's DNS and
        // port 853 (Private DNS) reaches the real resolver.
        dns: crate::xray::config::android_tunnel_dns(&repo.settings),
        ipv6: true,
        mtu: 1500,
        auto_connect: repo.settings.auto_connect,
    };
    EXPECTED.store(true, Ordering::Release);
    CRASHED.store(false, Ordering::Release);
    match bridge.start(&request).await {
        Ok(_) => Ok(()),
        Err(error) => {
            EXPECTED.store(false, Ordering::Release);
            Err(convert(error))
        }
    }
}

pub async fn stop() -> Result<()> {
    EXPECTED.store(false, Ordering::Release);
    CRASHED.store(false, Ordering::Release);
    bridge()?.stop().await.map(|_| ()).map_err(convert)
}

/// This process asked the service for a tunnel and has not stopped it.
pub fn active() -> bool {
    EXPECTED.load(Ordering::Acquire)
}

/// The tunnel this session started failed inside the service.
pub fn failed() -> bool {
    EXPECTED.load(Ordering::Acquire) && CRASHED.load(Ordering::Acquire)
}

/// Loads a configuration in the embedded core, like `xray run -test`.
pub async fn validate(config: &[u8], asset_dir: &str) -> std::result::Result<(), String> {
    let config = std::str::from_utf8(config).map_err(|_| "configuration".to_string())?;
    let bridge = bridge().map_err(|error| error.message)?;
    bridge
        .validate(config, asset_dir)
        .await
        .map_err(|error| error.message)
}

/// Starts a probe core with its own SOCKS inbound; `None` when it did not start.
pub async fn start_probe(config: &[u8], asset_dir: &str) -> Option<i64> {
    let config = std::str::from_utf8(config).ok()?;
    bridge().ok()?.start_probe(config, asset_dir).await.ok()
}

pub async fn stop_probe(id: i64) {
    if let Ok(bridge) = bridge() {
        let _ = bridge.stop_probe(id).await;
    }
}

pub async fn installed_apps() -> Result<Vec<AppItem>> {
    let apps = bridge()?.installed_apps().await.map_err(convert)?;
    Ok(apps
        .into_iter()
        .map(|app| AppItem {
            id: app.package.clone(),
            pid: 0,
            name: app.package,
            detail: app.label,
            icon: None,
        })
        .collect())
}

pub async fn app_icons(names: &[String]) -> Result<std::collections::HashMap<String, String>> {
    let packages: Vec<String> = names
        .iter()
        .flat_map(|name| name.split(','))
        .map(|name| name.trim().to_string())
        .filter(|name| is_package_name(name))
        .collect();
    if packages.is_empty() {
        return Ok(Default::default());
    }
    bridge()?.app_icons(&packages, 96).await.map_err(convert)
}

pub async fn open_url(url: &str) -> Result<()> {
    bridge()?.open_url(url).await.map_err(convert)
}

pub async fn move_to_background() -> Result<()> {
    bridge()?.move_to_background().await.map_err(convert)
}

pub async fn open_vpn_settings() -> Result<()> {
    bridge()?.open_vpn_settings().await.map_err(convert)
}

pub async fn battery_optimization_ignored() -> Result<bool> {
    bridge()?
        .battery_optimization_ignored()
        .await
        .map_err(convert)
}

pub async fn request_battery_exemption() -> Result<()> {
    bridge()?.request_battery_exemption().await.map_err(convert)
}

/// Same shape as the desktop process list, so the rule picker works unchanged:
/// `name` is the package (the rule value), `detail` the app label.
#[derive(serde::Serialize)]
pub struct AppItem {
    pub id: String,
    pub pid: u32,
    pub name: String,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

/// The 32-byte storage key RuntimeProvider unsealed from the Android Keystore.
pub fn storage_key() -> Option<zeroize::Zeroizing<[u8; 32]>> {
    let hex = std::env::var("LUMEN_STORAGE_KEY").ok()?;
    if hex.len() != 64 {
        return None;
    }
    let mut key = zeroize::Zeroizing::new([0u8; 32]);
    for (index, byte) in key.iter_mut().enumerate() {
        *byte = u8::from_str_radix(hex.get(index * 2..index * 2 + 2)?, 16).ok()?;
    }
    Some(key)
}
