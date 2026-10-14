use crate::{
    app::state::AppState,
    domain::errors::{BackendError, Result},
    services::catalog_service,
    storage::repository::RuleMatcher,
};
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};
use tauri::Emitter;

static IMPORT_LINK: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
pub fn queue_import_link(app: &tauri::AppHandle, input: &str) {
    if input.len() > 65536 || !input.starts_with("lumen://import?") {
        return;
    }
    if let Ok(mut pending) = IMPORT_LINK.lock() {
        *pending = Some(input.to_owned());
    }
    let _ = app.emit("import://pending", ());
}
#[tauri::command]
pub fn take_import_link() -> Option<String> {
    IMPORT_LINK.lock().ok()?.take()
}

#[tauri::command]
pub async fn export_backup(
    state: tauri::State<'_, Arc<AppState>>,
    password: String,
) -> Result<String> {
    let _guard = state.updates.lock().await;
    let repo = state.repository.read().await.clone();
    tokio::task::spawn_blocking(move || crate::storage::backup::encrypt(&repo, &password))
        .await
        .map_err(|_| BackendError::new("BACKUP_FAILED", "Backup operation failed"))?
}
#[tauri::command]
pub async fn preview_backup(
    state: tauri::State<'_, Arc<AppState>>,
    input: String,
    password: String,
) -> Result<Value> {
    let _guard = state.updates.lock().await;
    let repo =
        tokio::task::spawn_blocking(move || crate::storage::backup::decrypt(&input, &password))
            .await
            .map_err(|_| BackendError::new("BACKUP_FAILED", "Backup operation failed"))??;
    Ok(
        json!({"subscriptions":repo.subscriptions.len(),"servers":repo.nodes.len(),"rules":repo.rules.len()}),
    )
}
#[tauri::command]
pub async fn restore_backup(
    state: tauri::State<'_, Arc<AppState>>,
    app: tauri::AppHandle,
    input: String,
    password: String,
) -> Result<Value> {
    let _guard = state.updates.lock().await;
    let mut repo =
        tokio::task::spawn_blocking(move || crate::storage::backup::decrypt(&input, &password))
            .await
            .map_err(|_| BackendError::new("BACKUP_FAILED", "Backup operation failed"))??;
    if let Some(id) = &repo.selected_server_id {
        crate::core::backend(repo.settings.core_engine)
            .validate_runtime(&state, &repo, id)
            .await?;
    }
    crate::services::vpn_service::disconnect(&state, &app).await?;
    let _mutation = state.mutations.lock().await;
    // Autostart belongs to this installation; importing data cannot enable it or auto-connect.
    repo.settings.auto_start = state.repository.read().await.settings.auto_start;
    repo.settings.auto_connect = false;
    state.store.save(&repo)?;
    let catalog = catalog_service::snapshot(&repo, &Default::default());
    *state.repository.write().await = repo;
    state.latencies.write().await.clear();
    state.server_quality.write().await.clear();
    let _ = app.emit("catalog://changed", &catalog);
    crate::update_tray_menu(&app, &state);
    Ok(catalog)
}
#[tauri::command]
pub async fn get_maintenance_history(state: tauri::State<'_, Arc<AppState>>) -> Result<Value> {
    Ok(json!(state.repository.read().await.subscription_changes))
}
#[tauri::command]
pub async fn export_diagnostic_report(
    state: tauri::State<'_, Arc<AppState>>,
    app: tauri::AppHandle,
) -> Result<Value> {
    let repo = state.repository.read().await;
    let snapshot = state.snapshot.read().await;
    #[cfg(windows)]
    let routes = crate::platform::network::verify_tunnel_routes().is_ok();
    #[cfg(not(windows))]
    let routes = false;
    Ok(
        json!({"format":1,"applicationVersion":app.package_info().version.to_string(),
        "coreVersion":super::updates::version(&state.core.read().await),"platform":std::env::consts::OS,
        "status":snapshot.status,"errorCode":snapshot.error.as_ref().map(|e|e.code),
        "networkMode":repo.settings.network_mode,"routingMode":repo.routing_mode,"routesReady":routes,
        "dnsPolicyEnabled":repo.settings.dns_policy.enabled,"ipv6Policy":repo.settings.dns_policy.ipv6,
        "subscriptions":repo.subscriptions.len(),"servers":repo.nodes.len(),"rules":repo.rules.len(),
        "geoValid":crate::xray::geo::info(&state.data).valid}),
    )
}
fn destination(input: &str) -> Result<String> {
    let input = input.trim();
    let value = if input.contains("://") {
        input.to_owned()
    } else {
        format!("https://{input}")
    };
    let url = url::Url::parse(&value).map_err(|_| BackendError::invalid())?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some_and(|p| p != 443)
    {
        return Err(BackendError::invalid());
    }
    url.host_str()
        .filter(|h| h.len() <= 253)
        .map(str::to_owned)
        .ok_or_else(BackendError::invalid)
}
fn domain_match(rule: &str, host: &str) -> Option<bool> {
    if rule.contains(':') {
        return None;
    }
    let compiled = crate::xray::config::domain_rule(rule).ok()?;
    let host = host.to_ascii_lowercase();
    let value = compiled.strip_prefix("domain:")?;
    Some(host == value || host.ends_with(&format!(".{value}")))
}
#[tauri::command]
pub async fn inspect_destination(
    state: tauri::State<'_, Arc<AppState>>,
    input: String,
) -> Result<Value> {
    let host = destination(&input)?;
    let dns = tokio::time::timeout(
        Duration::from_secs(5),
        tokio::net::lookup_host((host.as_str(), 443)),
    )
    .await;
    let resolved = dns
        .ok()
        .and_then(std::result::Result::ok)
        .map(|items| items.count())
        .unwrap_or(0);
    let repo = state.repository.read().await.clone();
    let mut route = if repo.routing_mode == "global" {
        "proxy"
    } else {
        "direct"
    }
    .to_owned();
    let mut rule_id = None;
    let mut uncertain = repo.settings.bypass_lan;
    if repo.routing_mode == "rules" {
        for rule in catalog_service::ordered_rules(&repo.rules)
            .into_iter()
            .filter(|r| r.enabled)
        {
            let matched = match rule.matcher {
                RuleMatcher::Domain
                    if !rule.name.contains('/')
                        && !rule.name.starts_with("keyword:")
                        && !rule.name.starts_with("full:")
                        && rule.name.parse::<std::net::IpAddr>().is_err() =>
                {
                    domain_match(&rule.name, &host)
                }
                RuleMatcher::Final => Some(false),
                _ => None,
            };
            match matched {
                Some(true) => {
                    route = rule.server_id.clone();
                    rule_id = Some(rule.id.clone());
                    break;
                }
                None => uncertain = true,
                _ => {}
            }
        }
    }
    let port = {
        let session = state.session.lock().await;
        session.started().map(|_| session.proxy_port())
    };
    async fn check(host: &str, port: Option<u16>) -> Option<u16> {
        crate::services::health::client(port)
            .ok()?
            .head(format!("https://{host}/"))
            .send()
            .await
            .ok()
            .map(|r| r.status().as_u16())
    }
    let (system, proxy) = tokio::join!(check(&host, None), async {
        if port.is_some() {
            check(&host, port).await
        } else {
            None
        }
    });
    Ok(
        json!({"host":host,"resolvedAddresses":resolved,"systemHttpStatus":system,"proxyHttpStatus":proxy,
        "route":route,"ruleId":rule_id,"requiresCoreEvaluation":uncertain,"observed":false}),
    )
}
#[tauri::command]
pub async fn get_active_connections() -> Result<Value> {
    #[cfg(windows)]
    {
        let mut cmd = tokio::process::Command::new("powershell.exe");
        cmd.creation_flags(0x08000000).kill_on_drop(true);
        cmd.args(["-NoLogo","-NoProfile","-NonInteractive","-Command", "@((Get-NetTCPConnection -State Established -ErrorAction Stop | Select-Object -First 100) | ForEach-Object { $p=Get-Process -Id $_.OwningProcess -ErrorAction SilentlyContinue; [pscustomobject]@{process=$p.ProcessName;pid=$_.OwningProcess;address=$_.RemoteAddress;port=$_.RemotePort} }) | ConvertTo-Json -Compress"]);
        let output = tokio::time::timeout(Duration::from_secs(10), cmd.output())
            .await
            .map_err(|_| {
                BackendError::new("DIAGNOSTICS_FAILED", "Connection inspection timed out")
            })?
            .map_err(|_| BackendError::new("DIAGNOSTICS_FAILED", "Connection inspection failed"))?;
        if !output.status.success() || output.stdout.len() > 256 * 1024 {
            return Err(BackendError::new(
                "DIAGNOSTICS_FAILED",
                "Connection inspection failed",
            ));
        }
        let value: Value = serde_json::from_slice(&output.stdout).unwrap_or(json!([]));
        Ok(if value.is_array() {
            value
        } else if value.is_null() {
            json!([])
        } else {
            json!([value])
        })
    }
    #[cfg(not(windows))]
    {
        Ok(json!([]))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn destination_and_rule_matching_are_bounded() {
        assert!(destination("https://user:password@example.org").is_err());
        assert!(destination("http://example.org").is_err());
        assert_eq!(domain_match("*.example.org", "sub.example.org"), Some(true));
        assert_eq!(domain_match("example.org", "notexample.org"), Some(false));
        assert_eq!(domain_match("regexp:.*", "example.org"), None);
    }
}
