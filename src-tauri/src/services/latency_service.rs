use crate::{app::state::AppState, domain::errors::Result};
use std::time::{Duration, Instant};

static PROBES: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(4);
static PROBE_STARTUP: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub async fn ping(state: &AppState, id: String) -> Result<serde_json::Value> {
    let value = ping_inner(state, &id).await?;
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
async fn ping_inner(state: &AppState, id: &str) -> Result<serde_json::Value> {
    let _permit = match PROBES.acquire().await {
        Ok(p) => p,
        Err(_) => return Ok(serde_json::json!("timeout")),
    };

    let mut repo = state.repository.read().await.clone();
    // Port discovery releases its temporary listeners before Xray binds them.
    // Serialize only that short startup window so concurrent probes cannot be
    // assigned the same just-released Windows ephemeral ports.
    // A probe only needs its SOCKS inbound and the node's outbound. With routing
    // rules each probe loaded the ~620k-domain antifilter list (seconds of CPU and
    // ~120 MB per process), and 25 of them at startup starved the tunnel.
    repo.rules.clear();
    repo.routing_mode = "global".into();
    repo.settings.dns_policy.enabled = false;
    if repo.settings.dns_server == "system" {
        repo.settings.dns_server = crate::storage::repository::Settings::default().dns_server;
    }
    let startup = PROBE_STARTUP.lock().await;
    let (proxy, metrics) = match crate::engine::xray::ports() {
        Ok(p) => p,
        Err(_) => return Ok(serde_json::json!("timeout")),
    };
    let config = match crate::xray::config::build(&repo, id, proxy, metrics, false) {
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

    let result = if ready {
        probe_target(proxy, repo.settings.ping_url.trim()).await
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

/// Whole-probe budget, including the cold first request through the tunnel.
const PROBE_BUDGET: Duration = Duration::from_secs(5);
/// Timed requests sent over the warmed-up connection.
const PROBE_SAMPLES: usize = 2;

/// Measures the real round trip through the node the way v2rayN does it.
///
/// The first request pays for the SOCKS handshake, the outbound's own handshake
/// (TLS/REALITY/etc.) and the TCP/TLS setup to the target, so it says little
/// about the node's latency. It only warms the connection up; the result is the
/// fastest of the following requests, which reuse that keep-alive connection
/// and measure one HTTP exchange through the tunnel.
async fn probe_target(proxy_port: u16, custom_url: &str) -> Option<u64> {
    let proxy = reqwest::Proxy::all(format!("socks5h://127.0.0.1:{proxy_port}")).ok()?;
    let client = reqwest::Client::builder()
        .no_proxy()
        .proxy(proxy)
        .redirect(reqwest::redirect::Policy::none())
        .tcp_nodelay(true)
        .pool_max_idle_per_host(1)
        .timeout(PROBE_BUDGET)
        .build()
        .ok()?;
    let deadline = tokio::time::Instant::now() + PROBE_BUDGET;

    for endpoint in std::iter::once(custom_url)
        .filter(|url| !url.is_empty())
        .chain([
            "https://www.google.com/generate_204",
            "https://cp.cloudflare.com/generate_204",
        ])
    {
        let Ok(Some(cold)) = tokio::time::timeout_at(deadline, request(&client, endpoint)).await
        else {
            if tokio::time::Instant::now() >= deadline {
                return None;
            }
            continue;
        };

        let mut best = None;
        for _ in 0..PROBE_SAMPLES {
            let pause = tokio::time::sleep(Duration::from_millis(100));
            if tokio::time::timeout_at(deadline, pause).await.is_err() {
                break;
            }
            match tokio::time::timeout_at(deadline, request(&client, endpoint)).await {
                Ok(Some(elapsed)) => best = Some(best.map_or(elapsed, |b: u64| b.min(elapsed))),
                Ok(None) => {}
                Err(_) => break,
            }
        }
        return Some(best.unwrap_or(cold).max(1));
    }
    None
}

/// Sends one request and returns the time until the response headers arrived,
/// or `None` if it failed or the status shows the target was not reached.
async fn request(client: &reqwest::Client, endpoint: &str) -> Option<u64> {
    let start = Instant::now();
    let res = client.get(endpoint).send().await.ok()?;
    let elapsed = start.elapsed().as_millis() as u64;
    let status = res.status().as_u16();
    let ok = if endpoint.ends_with("/generate_204") {
        status == 204
    } else {
        (200..=299).contains(&status)
    };
    // Drain the body so the connection goes back to the pool for the next sample.
    let _ = res.bytes().await;
    ok.then_some(elapsed)
}
