use crate::{app::state::AppState, domain::errors::Result};
use std::time::{Duration, Instant};

static PROBES: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(4);
static PROBE_STARTUP: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub async fn ping(state: &AppState, id: String) -> Result<serde_json::Value> {
    let value = ping_inner(state, id.clone()).await?;
    state
        .latencies
        .write()
        .await
        .insert(id.clone(), value.clone());
    if state
        .repository
        .read()
        .await
        .nodes
        .iter()
        .any(|node| node.id == id)
    {
        state
            .server_quality
            .write()
            .await
            .entry(id)
            .or_default()
            .record(value.as_u64());
    }
    Ok(value)
}
async fn ping_inner(state: &AppState, id: String) -> Result<serde_json::Value> {
    let _permit = match PROBES.acquire().await {
        Ok(p) => p,
        Err(_) => return Ok(serde_json::json!("timeout")),
    };

    let repo = state.repository.read().await.clone();
    // Port discovery releases its temporary listeners before Xray binds them.
    // Serialize only that short startup window so concurrent probes cannot be
    // assigned the same just-released Windows ephemeral ports.
    let startup = PROBE_STARTUP.lock().await;
    let (proxy, metrics) = match super::vpn_service::ports() {
        Ok(p) => p,
        Err(_) => return Ok(serde_json::json!("timeout")),
    };
    let config = match crate::xray::config::build(&repo, &id, proxy, metrics, false) {
        Ok(c) => c,
        Err(_) => return Ok(serde_json::json!("timeout")),
    };
    let bytes = match serde_json::to_vec(&config) {
        Ok(b) => b,
        Err(_) => return Ok(serde_json::json!("timeout")),
    };
    let core = state.core.read().await.clone();

    #[cfg(windows)]
    let mut child = match crate::platform::process::CoreProcess::spawn(&core, &bytes).await {
        Ok(c) => c,
        Err(_) => return Ok(serde_json::json!("timeout")),
    };
    #[cfg(not(windows))]
    return Ok(serde_json::json!("timeout"));

    // Wait for the local SOCKS proxy port to accept connections
    let mut ready = false;
    for _ in 0..20 {
        if child.try_wait().map(|w| w.is_some()).unwrap_or(true) {
            break;
        }
        if tokio::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, proxy))
            .await
            .is_ok()
        {
            ready = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    drop(startup);

    let ping_url = repo.settings.ping_url.trim().to_string();
    let result = if ready {
        probe_target(proxy, &ping_url).await
    } else {
        None
    };

    let _ = child.kill().await;

    let val = match result {
        Some(elapsed) => serde_json::json!(elapsed),
        None => serde_json::json!("timeout"),
    };

    Ok(val)
}

async fn probe_target(proxy_port: u16, custom_url: &str) -> Option<u64> {
    let proxy = reqwest::Proxy::all(format!("socks5h://127.0.0.1:{proxy_port}")).ok()?;
    let client = reqwest::Client::builder()
        .no_proxy()
        .proxy(proxy)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_millis(2500))
        .build()
        .ok()?;

    let mut endpoints: Vec<&str> = Vec::new();
    if !custom_url.is_empty() {
        endpoints.push(custom_url);
    }
    endpoints.push("https://www.google.com/generate_204");
    endpoints.push("https://cp.cloudflare.com/generate_204");

    for endpoint in endpoints {
        let start = Instant::now();
        if let Ok(res) = client.get(endpoint).send().await {
            let status = res.status().as_u16();
            if if endpoint.ends_with("/generate_204") {
                status == 204
            } else {
                (200..=299).contains(&status)
            } {
                return Some(start.elapsed().as_millis() as u64);
            }
        }
    }
    None
}
