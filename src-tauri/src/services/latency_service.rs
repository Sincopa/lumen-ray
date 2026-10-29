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
    #[cfg(any(windows, target_os = "macos", target_os = "android"))]
    {
        let Some(core) = isolated_core(state, id).await else {
            return Ok(serde_json::json!("timeout"));
        };
        let ping_url = state.repository.read().await.settings.ping_url.clone();
        let result = probe_target(core.proxy_port, ping_url.trim()).await;
        core.stop().await;
        Ok(match result {
            Some(elapsed) => serde_json::json!(elapsed),
            None => serde_json::json!("timeout"),
        })
    }
    #[cfg(not(any(windows, target_os = "macos", target_os = "android")))]
    {
        let _ = (state, id);
        Ok(serde_json::json!("timeout"))
    }
}

/// A short-lived core that serves one node on a local SOCKS port (TCP and UDP).
/// Dropping it ends the process with its job object.
#[cfg(any(windows, target_os = "macos"))]
pub(crate) struct IsolatedCore {
    pub proxy_port: u16,
    child: crate::platform::process::CoreProcess,
    _permit: tokio::sync::SemaphorePermit<'static>,
}

#[cfg(any(windows, target_os = "macos"))]
impl IsolatedCore {
    pub async fn stop(mut self) {
        let _ = self.child.kill().await;
    }
}

/// On Android the probe is a second Xray instance inside the app process,
/// next to the tunnel; Lumen's own sockets bypass the VPN.
#[cfg(target_os = "android")]
pub(crate) struct IsolatedCore {
    pub proxy_port: u16,
    probe: i64,
    _permit: tokio::sync::SemaphorePermit<'static>,
}

#[cfg(target_os = "android")]
impl IsolatedCore {
    pub async fn stop(self) {
        crate::platform::android::stop_probe(self.probe).await;
    }
}

/// Same reduced configuration as on desktop, with only the authenticated
/// `probe` inbound: no HTTP proxy is opened on loopback.
#[cfg(target_os = "android")]
pub(crate) async fn isolated_core(state: &AppState, id: &str) -> Option<IsolatedCore> {
    let permit = PROBES.acquire().await.ok()?;
    let mut repo = state.repository.read().await.clone();
    repo.rules.clear();
    repo.routing_mode = "global".into();
    repo.discord_voice_routes.clear();
    repo.settings.dns_policy.enabled = false;
    if repo.settings.dns_server == "system" {
        repo.settings.dns_server = crate::storage::repository::Settings::default().dns_server;
    }
    let assets = crate::xray::geo::assets_for_core(&state.core.read().await);
    let _startup = PROBE_STARTUP.lock().await;
    // A port another app took between discovery and bind fails the start; new ports fix it.
    for _ in 0..2 {
        let (proxy, metrics) = crate::engine::xray::ports().ok()?;
        let config = crate::xray::config::build(&repo, id, proxy, metrics, false).ok()?;
        let mut config = serde_json::to_value(config).ok()?;
        if let Some(inbounds) = config
            .get_mut("inbounds")
            .and_then(serde_json::Value::as_array_mut)
        {
            inbounds.retain(|inbound| inbound["tag"] == "probe");
        }
        let bytes = serde_json::to_vec(&config).ok()?;
        // The embedded core has bound its inbound when start_probe returns.
        if let Some(probe) =
            crate::platform::android::start_probe(&bytes, &assets.to_string_lossy()).await
        {
            return Some(IsolatedCore {
                proxy_port: proxy,
                probe,
                _permit: permit,
            });
        }
    }
    None
}

#[cfg(any(windows, target_os = "macos"))]
pub(crate) async fn isolated_core(state: &AppState, id: &str) -> Option<IsolatedCore> {
    let permit = PROBES.acquire().await.ok()?;
    let mut repo = state.repository.read().await.clone();
    // A probe only needs its SOCKS inbound and the node's outbound. With routing
    // rules each probe loaded the ~620k-domain antifilter list (seconds of CPU and
    // ~120 MB per process), and 25 of them at startup starved the tunnel.
    repo.rules.clear();
    repo.routing_mode = "global".into();
    repo.discord_voice_routes.clear();
    repo.settings.dns_policy.enabled = false;
    if repo.settings.dns_server == "system" {
        repo.settings.dns_server = crate::storage::repository::Settings::default().dns_server;
    }
    // Port discovery releases its temporary listeners before Xray binds them.
    // Serialize only that short startup window so concurrent probes cannot be
    // assigned the same just-released Windows ephemeral ports.
    let startup = PROBE_STARTUP.lock().await;
    let (proxy, metrics) = crate::engine::xray::ports().ok()?;
    let config = crate::xray::config::build(&repo, id, proxy, metrics, false).ok()?;
    let bytes = serde_json::to_vec(&config).ok()?;
    let core = state.core.read().await.clone();
    let mut child = crate::platform::process::CoreProcess::spawn(&core, &bytes)
        .await
        .ok()?;

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
    if !ready {
        let _ = child.kill().await;
        return None;
    }
    Some(IsolatedCore {
        proxy_port: proxy,
        child,
        _permit: permit,
    })
}

/// Whole-probe budget, including the cold first request through the tunnel.
/// Mobile networks add a second or two to the REALITY/TLS handshake.
const PROBE_BUDGET: Duration = Duration::from_secs(if cfg!(target_os = "android") { 8 } else { 5 });
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
    let proxy = crate::xray::probe::proxy(proxy_port).ok()?;
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
        // Some exits never answer plain HTTP on port 80 (the default ping URL).
        // On Android a stalled first endpoint leaves time for the HTTPS ones.
        let cold_deadline = if cfg!(target_os = "android") {
            deadline.min(tokio::time::Instant::now() + PROBE_BUDGET / 2)
        } else {
            deadline
        };
        let Ok(Some(cold)) =
            tokio::time::timeout_at(cold_deadline, request(&client, endpoint)).await
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
