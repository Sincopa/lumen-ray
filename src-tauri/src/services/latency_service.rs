use crate::{
    app::state::AppState,
    domain::errors::Result,
};
use std::time::{Duration, Instant};

static PROBES: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(4);

pub async fn ping(state: &AppState, id: String) -> Result<serde_json::Value> {
    let _permit = match PROBES.acquire().await {
        Ok(p) => p,
        Err(_) => return Ok(serde_json::json!("timeout")),
    };

    let repo = state.repository.read().await.clone();
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

    let ping_url = repo.settings.ping_url.trim().to_string();
    let result = if ready {
        probe_target(proxy, &ping_url).await
    } else {
        None
    };

    let _ = child.kill();

    let val = match result {
        Some(elapsed) => serde_json::json!(elapsed),
        None => serde_json::json!("timeout"),
    };

    // Cache the measured latency in AppState so it is never lost on navigation
    state.latencies.write().await.insert(id, val.clone());

    Ok(val)
}

async fn probe_target(proxy_port: u16, custom_url: &str) -> Option<u64> {
    let proxy = reqwest::Proxy::all(format!("socks5h://127.0.0.1:{proxy_port}")).ok()?;
    let client = reqwest::Client::builder()
        .no_proxy()
        .proxy(proxy)
        .timeout(Duration::from_millis(2500))
        .build()
        .ok()?;

    let mut endpoints: Vec<&str> = Vec::new();
    if !custom_url.is_empty() {
        endpoints.push(custom_url);
    }
    endpoints.push("http://www.google.com/generate_204");
    endpoints.push("http://cp.cloudflare.com/generate_204");

    for endpoint in endpoints {
        let start = Instant::now();
        if let Ok(res) = client.get(endpoint).send().await {
            let status = res.status().as_u16();
            if (200..=299).contains(&status) || status == 301 || status == 302 {
                return Some(start.elapsed().as_millis() as u64);
            }
        }
    }
    None
}
