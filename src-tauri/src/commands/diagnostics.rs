use crate::{
    app::state::AppState,
    domain::errors::{BackendError, Result},
};
use serde::Serialize;
use std::{sync::Arc, time::Duration};
use tauri::Manager;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkDiagnostics {
    status: String,
    network_mode: String,
    core_version: String,
    core_engine: crate::engine::CoreEngine,
    selected_core_engine: crate::engine::CoreEngine,
    public_ipv4: Option<String>,
    public_ipv6: Option<String>,
    system_dns: Vec<String>,
    configured_dns: String,
    tunnel_ready: bool,
    routes_ready: bool,
    routing_conflict: bool,
    system_proxy_enabled: bool,
    proxy_managed: bool,
}

async fn public_ip(client: &reqwest::Client, endpoint: &'static str) -> Option<String> {
    let text = client.get(endpoint).send().await.ok()?.text().await.ok()?;
    text.trim()
        .parse::<std::net::IpAddr>()
        .ok()
        .map(|ip| ip.to_string())
}

#[tauri::command]
pub async fn get_network_diagnostics(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<NetworkDiagnostics> {
    let snapshot = state.snapshot.read().await.clone();
    let settings = state.repository.read().await.settings.clone();
    let session = state.session.lock().await;
    let port = session.started().map(|_| session.proxy_port());
    let active_engine = session
        .started()
        .map(|_| session.engine())
        .unwrap_or(settings.core_engine);
    let network_mode = if session.started().is_some() {
        if session.is_tun() { "tun" } else { "proxy" }.to_string()
    } else {
        settings.network_mode.clone()
    };
    #[cfg(windows)]
    let (system_dns, tunnel_ready, routes_ready, system_proxy_enabled, proxy_managed) = (
        crate::platform::network::system_dns().unwrap_or_default(),
        crate::platform::network::tunnel_ready(),
        network_mode == "tun"
            && snapshot.status == "connected"
            && crate::platform::network::verify_tunnel_routes().is_ok(),
        crate::platform::proxy::is_enabled().unwrap_or(false),
        session.system_proxy.is_some(),
    );
    #[cfg(not(windows))]
    let (system_dns, tunnel_ready, routes_ready, system_proxy_enabled, proxy_managed) =
        (Vec::new(), false, false, false, false);
    drop(session);
    let (public_ipv4, public_ipv6) = if let Some(port) = port {
        let client = reqwest::Proxy::all(format!("socks5h://127.0.0.1:{port}"))
            .ok()
            .and_then(|proxy| {
                reqwest::Client::builder()
                    .no_proxy()
                    .proxy(proxy)
                    .timeout(Duration::from_secs(3))
                    .build()
                    .ok()
            });
        if let Some(client) = client {
            tokio::join!(
                public_ip(&client, "https://api.ipify.org"),
                public_ip(&client, "https://api6.ipify.org"),
            )
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };
    let routing_conflict =
        network_mode == "tun" && snapshot.status == "connected" && tunnel_ready && !routes_ready;
    Ok(NetworkDiagnostics {
        status: snapshot.status,
        network_mode,
        core_engine: active_engine,
        selected_core_engine: settings.core_engine,
        core_version: super::updates::version(&state.core.read().await),
        public_ipv4,
        public_ipv6,
        system_dns,
        configured_dns: settings.dns_server,
        tunnel_ready,
        routes_ready,
        routing_conflict,
        system_proxy_enabled,
        proxy_managed,
    })
}

#[tauri::command]
pub fn open_log_console(app: tauri::AppHandle) -> Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        use windows_sys::Win32::System::Threading::CREATE_NEW_CONSOLE;

        let log_path = app
            .path()
            .app_log_dir()
            .map_err(|_| BackendError::new("LOG_CONSOLE_FAILED", "Log directory unavailable"))?
            .join(format!("{}.log", app.package_info().name));
        log::info!("Opening live log console");
        let script = concat!(
            "$Host.UI.RawUI.WindowTitle = 'Lumen logs'; ",
            "Write-Host 'Lumen - live application logs' -ForegroundColor Cyan; ",
            "Write-Host $env:LUMEN_LOG_PATH -ForegroundColor DarkGray; ",
            "Write-Host 'Close this window when testing is finished.' -ForegroundColor DarkGray; ",
            "while (-not (Test-Path -LiteralPath $env:LUMEN_LOG_PATH)) { Start-Sleep -Milliseconds 100 }; ",
            "Get-Content -LiteralPath $env:LUMEN_LOG_PATH -Tail 200 -Wait"
        );
        std::process::Command::new("powershell.exe")
            .args(["-NoLogo", "-NoProfile", "-Command", script])
            .env("LUMEN_LOG_PATH", log_path)
            .creation_flags(CREATE_NEW_CONSOLE)
            .spawn()
            .map(|_| ())
            .map_err(|_| BackendError::new("LOG_CONSOLE_FAILED", "Could not open log console"))
    }
    #[cfg(not(windows))]
    {
        let _ = app;
        Err(BackendError::new(
            "LOG_CONSOLE_FAILED",
            "Log console is only available on Windows",
        ))
    }
}
