//! Bridge from the Lumen Rust core to the Android side: VPN consent, the
//! foreground `VpnService` that runs Xray in-process, installed apps, battery
//! settings. Every call is async; the Kotlin command runs on the main thread,
//! so a blocking call from the main thread would deadlock.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};

#[cfg(target_os = "android")]
const PLUGIN_IDENTIFIER: &str = "app.lumen.plugin";

/// A refusal from the Android side, with the same codes the desktop core uses
/// (`VPN_PERMISSION_DENIED`, `CORE_START_FAILED`, `INVALID_CONFIG`, ...).
#[derive(Debug, Clone)]
pub struct Error {
    pub code: String,
    pub message: String,
}

impl Error {
    fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

/// Everything the service needs to bring the tunnel up again without the UI:
/// Always-on VPN, the Quick Settings tile and boot start reuse the last plan.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartRequest {
    /// Complete Xray JSON with the `tun` inbound.
    pub config: String,
    /// Directory with geoip.dat and geosite.dat.
    pub asset_dir: String,
    /// Packages that bypass the VPN. Ignored when `included_apps` is not empty.
    pub excluded_apps: Vec<String>,
    /// When not empty, only these packages use the VPN.
    pub included_apps: Vec<String>,
    /// Shown in the persistent notification.
    pub server_name: String,
    /// The DNS address announced to apps; Xray answers it inside the tunnel.
    pub dns: String,
    pub ipv6: bool,
    pub mtu: u32,
    /// Bring the tunnel up after a reboot when the user enabled auto-connect.
    pub auto_connect: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// `idle`, `connecting`, `connected`, `stopping` or `error`.
    pub state: String,
    pub running: bool,
    #[serde(default)]
    pub error_code: Option<String>,
    #[serde(default)]
    pub error_message: Option<String>,
    /// Unix milliseconds of the current connection.
    #[serde(default)]
    pub started_at: Option<u64>,
    #[serde(default)]
    pub server_name: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Stats {
    pub up: u64,
    pub down: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledApp {
    pub package: String,
    pub label: String,
    pub system: bool,
}

#[derive(Deserialize)]
struct Granted {
    granted: bool,
}

#[derive(Deserialize)]
struct Ignored {
    ignored: bool,
}

#[derive(Deserialize)]
struct Apps {
    apps: Vec<InstalledApp>,
}

#[derive(Deserialize)]
struct Icons {
    icons: HashMap<String, String>,
}

#[derive(Deserialize)]
struct Version {
    version: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ValidateRequest<'a> {
    config: &'a str,
    asset_dir: &'a str,
}

#[derive(Serialize)]
struct IconsRequest<'a> {
    packages: &'a [String],
    size: u32,
}

#[cfg(target_os = "android")]
pub struct Lumen<R: Runtime>(tauri::plugin::PluginHandle<R>);

#[cfg(not(target_os = "android"))]
pub struct Lumen<R: Runtime>(std::marker::PhantomData<fn() -> R>);

impl<R: Runtime> Clone for Lumen<R> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<R: Runtime> Lumen<R> {
    #[cfg(target_os = "android")]
    async fn call<T: serde::de::DeserializeOwned>(
        &self,
        command: &str,
        payload: impl Serialize,
    ) -> Result<T> {
        use tauri::plugin::mobile::PluginInvokeError;
        self.0
            .run_mobile_plugin_async(command, payload)
            .await
            .map_err(|error| match error {
                PluginInvokeError::InvokeRejected(response) => Error::new(
                    response.code.as_deref().unwrap_or("ANDROID_FAILED"),
                    response.message.unwrap_or_default(),
                ),
                other => Error::new("ANDROID_FAILED", other.to_string()),
            })
    }

    #[cfg(not(target_os = "android"))]
    async fn call<T: serde::de::DeserializeOwned>(
        &self,
        _command: &str,
        _payload: impl Serialize,
    ) -> Result<T> {
        Err(Error::new(
            "PLATFORM_UNSUPPORTED",
            "The Lumen Android bridge only runs on Android",
        ))
    }

    /// Shows the system VPN consent dialog when needed. `true` once Lumen may
    /// establish a VPN; `false` when the user declined.
    pub async fn prepare(&self) -> Result<bool> {
        Ok(self.call::<Granted>("prepare", ()).await?.granted)
    }

    /// Starts or replaces the tunnel and resolves once Xray is running.
    pub async fn start(&self, request: &StartRequest) -> Result<Status> {
        self.call("start", request).await
    }

    pub async fn stop(&self) -> Result<Status> {
        self.call("stop", ()).await
    }

    pub async fn status(&self) -> Result<Status> {
        self.call("status", ()).await
    }

    pub async fn stats(&self) -> Result<Stats> {
        self.call("stats", ()).await
    }

    /// Loads the configuration in the embedded core without starting it.
    pub async fn validate(&self, config: &str, asset_dir: &str) -> Result<()> {
        self.call::<serde_json::Value>("validate", ValidateRequest { config, asset_dir })
            .await
            .map(|_| ())
    }

    /// Starts a short-lived second core next to the tunnel (latency checks);
    /// returns its id for [`Self::stop_probe`].
    pub async fn start_probe(&self, config: &str, asset_dir: &str) -> Result<i64> {
        #[derive(Deserialize)]
        struct Probe {
            id: i64,
        }
        Ok(self
            .call::<Probe>("startProbe", ValidateRequest { config, asset_dir })
            .await?
            .id)
    }

    pub async fn stop_probe(&self, id: i64) -> Result<()> {
        #[derive(Serialize)]
        struct Probe {
            id: i64,
        }
        self.call::<serde_json::Value>("stopProbe", Probe { id })
            .await
            .map(|_| ())
    }

    pub async fn installed_apps(&self) -> Result<Vec<InstalledApp>> {
        Ok(self.call::<Apps>("installedApps", ()).await?.apps)
    }

    /// PNG data URLs by package name.
    pub async fn app_icons(
        &self,
        packages: &[String],
        size: u32,
    ) -> Result<HashMap<String, String>> {
        Ok(self
            .call::<Icons>("appIcons", IconsRequest { packages, size })
            .await?
            .icons)
    }

    pub async fn core_version(&self) -> Result<String> {
        Ok(self.call::<Version>("coreVersion", ()).await?.version)
    }

    /// Opens an https link in the browser.
    pub async fn open_url(&self, url: &str) -> Result<()> {
        #[derive(Serialize)]
        struct OpenUrl<'a> {
            url: &'a str,
        }
        self.call::<serde_json::Value>("openUrl", OpenUrl { url })
            .await
            .map(|_| ())
    }

    /// Moves Lumen's task to the background; the VPN service keeps running.
    pub async fn move_to_background(&self) -> Result<()> {
        self.call::<serde_json::Value>("moveToBackground", ())
            .await
            .map(|_| ())
    }

    /// Opens Android VPN settings: Always-on VPN and "Block connections without VPN".
    pub async fn open_vpn_settings(&self) -> Result<()> {
        self.call::<serde_json::Value>("openVpnSettings", ())
            .await
            .map(|_| ())
    }

    pub async fn battery_optimization_ignored(&self) -> Result<bool> {
        Ok(self
            .call::<Ignored>("batteryOptimization", ())
            .await?
            .ignored)
    }

    pub async fn request_battery_exemption(&self) -> Result<()> {
        self.call::<serde_json::Value>("requestBatteryExemption", ())
            .await
            .map(|_| ())
    }
}

pub trait LumenExt<R: Runtime> {
    fn lumen(&self) -> &Lumen<R>;
}

impl<R: Runtime, T: Manager<R>> LumenExt<R> for T {
    fn lumen(&self) -> &Lumen<R> {
        self.state::<Lumen<R>>().inner()
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("lumen")
        .setup(|app, _api| {
            #[cfg(target_os = "android")]
            let lumen = Lumen(_api.register_android_plugin(PLUGIN_IDENTIFIER, "LumenPlugin")?);
            #[cfg(not(target_os = "android"))]
            let lumen = Lumen::<R>(std::marker::PhantomData);
            app.manage(lumen);
            Ok(())
        })
        .build()
}
