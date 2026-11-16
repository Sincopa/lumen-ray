use crate::{app::state::AppState, domain::errors::Result};
use std::time::{Duration, Instant};

static PROBES: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(4);
static PROBE_STARTUP: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub async fn ping(state: &AppState, id: String) -> Result<serde_json::Value> {
    let (value, lead) = ping_inner(state, &id).await?;
    if let Some(lead) = lead {
        if let Err(error) = super::catalog_service::set_balancer_lead(state, &id, &lead).await {
            log::warn!(
                "Could not save the auto server's lead member: {}",
                error.code
            );
        }
    }
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
/// The latency of `id` and, for an auto server whose lead member should
/// change, the new lead.
async fn ping_inner(state: &AppState, id: &str) -> Result<(serde_json::Value, Option<String>)> {
    #[cfg(any(windows, target_os = "macos", target_os = "android"))]
    {
        let Some(core) = isolated_core(state, id, true).await else {
            return Ok((serde_json::json!("timeout"), None));
        };
        let ping_url = state.repository.read().await.settings.ping_url.clone();
        let members = core.members.clone();
        let ports: Vec<u16> = if members.is_empty() {
            vec![core.proxy_port]
        } else {
            members.iter().map(|(_, port)| *port).collect()
        };
        let elapsed = timings(&ports, ping_url.trim()).await;
        core.stop().await;
        let lead = if members.is_empty() {
            None
        } else {
            let current = state
                .repository
                .read()
                .await
                .balancer_leads
                .get(id)
                .cloned();
            let timed: Vec<(String, Option<u64>)> = members
                .into_iter()
                .map(|(tag, _)| tag)
                .zip(elapsed.iter().copied())
                .collect();
            crate::xray::config::next_lead(current.as_deref(), &timed)
                .filter(|lead| current.as_ref() != Some(lead))
        };
        let value = match elapsed.into_iter().flatten().min() {
            Some(elapsed) => serde_json::json!(elapsed),
            None => serde_json::json!("timeout"),
        };
        Ok((value, lead))
    }
    #[cfg(not(any(windows, target_os = "macos", target_os = "android")))]
    {
        let _ = (state, id);
        Ok((serde_json::json!("timeout"), None))
    }
}

/// A short-lived core that serves one node on a local SOCKS port (TCP and UDP).
/// Dropping it ends the process with its job object.
#[cfg(any(windows, target_os = "macos"))]
pub(crate) struct IsolatedCore {
    pub proxy_port: u16,
    /// Each auto-server member with the port routed straight to it; empty for
    /// a plain server or when members were not requested.
    pub members: Vec<(String, u16)>,
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
    pub members: Vec<(String, u16)>,
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
pub(crate) async fn isolated_core(
    state: &AppState,
    id: &str,
    members: bool,
) -> Option<IsolatedCore> {
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
    let tags = if members {
        member_tags(&repo, id)
    } else {
        Vec::new()
    };
    for _ in 0..2 {
        let (proxy, metrics) = crate::engine::xray::ports().ok()?;
        let member_ports = probe_ports(tags.len(), &core_ports(proxy, metrics))?;
        let config = crate::xray::config::build(&repo, id, proxy, metrics, false).ok()?;
        let mut config = serde_json::to_value(config).ok()?;
        if let Some(inbounds) = config
            .get_mut("inbounds")
            .and_then(serde_json::Value::as_array_mut)
        {
            inbounds.retain(|inbound| inbound["tag"] == "probe");
        }
        let members = paired(&tags, &member_ports);
        crate::xray::config::add_member_probes(&mut config, &members);
        let bytes = serde_json::to_vec(&config).ok()?;
        // The embedded core has bound its inbound when start_probe returns.
        if let Some(probe) =
            crate::platform::android::start_probe(&bytes, &assets.to_string_lossy()).await
        {
            return Some(IsolatedCore {
                proxy_port: proxy,
                members,
                probe,
                _permit: permit,
            });
        }
    }
    None
}

#[cfg(any(windows, target_os = "macos"))]
pub(crate) async fn isolated_core(
    state: &AppState,
    id: &str,
    members: bool,
) -> Option<IsolatedCore> {
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
    let tags = if members {
        member_tags(&repo, id)
    } else {
        Vec::new()
    };
    let startup = PROBE_STARTUP.lock().await;
    let (proxy, metrics) = crate::engine::xray::ports().ok()?;
    let member_ports = probe_ports(tags.len(), &core_ports(proxy, metrics))?;
    let config = crate::xray::config::build(&repo, id, proxy, metrics, false).ok()?;
    let mut config = serde_json::to_value(config).ok()?;
    // Only the `probe` inbound, as on Android. The system-proxy inbound opened an
    // HTTP proxy on loopback for every probe, on metrics + 1.
    if let Some(inbounds) = config
        .get_mut("inbounds")
        .and_then(serde_json::Value::as_array_mut)
    {
        inbounds.retain(|inbound| inbound["tag"] == "probe");
    }
    let members = paired(&tags, &member_ports);
    crate::xray::config::add_member_probes(&mut config, &members);
    // With the tunnel up, the probe must not travel through it: see bind_outbounds.
    #[cfg(windows)]
    {
        if crate::platform::network::tunnel_ready() {
            if let Some(interface) = crate::platform::network::physical_interface() {
                crate::xray::config::bind_outbounds(&mut config, &interface);
            }
        }
    }
    let bytes = serde_json::to_vec(&config).ok()?;
    let core = state.core.read().await.clone();
    let mut child = crate::platform::process::CoreProcess::spawn(&core, &bytes)
        .await
        .ok()?;

    // Wait for the local SOCKS ports to accept connections
    let mut ready = false;
    let mut waiting: Vec<u16> = std::iter::once(proxy)
        .chain(member_ports.iter().copied())
        .collect();
    for _ in 0..20 {
        if child.try_wait().map(|w| w.is_some()).unwrap_or(true) {
            break;
        }
        while let Some(&port) = waiting.first() {
            if tokio::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port))
                .await
                .is_err()
            {
                break;
            }
            waiting.remove(0);
        }
        if waiting.is_empty() {
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
        members,
        child,
        _permit: permit,
    })
}

/// The members of `id` to time one by one, or none for a plain server.
#[cfg(any(windows, target_os = "macos", target_os = "android"))]
fn member_tags(repo: &crate::storage::repository::Repository, id: &str) -> Vec<String> {
    repo.nodes
        .iter()
        .find(|node| node.id == id)
        .and_then(|node| node.balancer.as_ref())
        .map(crate::xray::config::balancer_members)
        .unwrap_or_default()
}

#[cfg(any(windows, target_os = "macos", target_os = "android"))]
fn paired(tags: &[String], ports: &[u16]) -> Vec<(String, u16)> {
    tags.iter().cloned().zip(ports.iter().copied()).collect()
}

/// The ports a probe core takes besides its member ports: the SOCKS port, the
/// metrics port and the two after it that `ports()` keeps free (HTTP and API).
#[cfg(any(windows, target_os = "macos", target_os = "android"))]
fn core_ports(proxy: u16, metrics: u16) -> [u16; 4] {
    [
        proxy,
        metrics,
        metrics.saturating_add(1),
        metrics.saturating_add(2),
    ]
}

/// `count` distinct free loopback ports, none of them in `taken`. The listeners
/// are held together and released at once, so Windows cannot hand the same
/// ephemeral port out twice. `ports()` has just released the ports in `taken`,
/// and Windows hands ports out in order: the first member port used to be
/// metrics + 1, Xray failed with «local port already in use», and every auto
/// server showed no ping on Windows.
#[cfg(any(windows, target_os = "macos", target_os = "android"))]
pub(crate) fn probe_ports(count: usize, taken: &[u16]) -> Option<Vec<u16>> {
    let mut held = Vec::with_capacity(count + taken.len());
    let mut ports = Vec::with_capacity(count);
    while ports.len() < count {
        if held.len() >= count + taken.len() + 16 {
            return None;
        }
        let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).ok()?;
        let port = listener.local_addr().ok()?.port();
        if !taken.contains(&port) {
            ports.push(port);
        }
        held.push(listener);
    }
    Some(ports)
}

/// Times every port at once, in the order given. An auto server is as fast as
/// its best member, which a leastPing balancer picks once it has data.
#[cfg(any(windows, target_os = "macos", target_os = "android"))]
async fn timings(ports: &[u16], custom_url: &str) -> Vec<Option<u64>> {
    let mut probes = tokio::task::JoinSet::new();
    for (index, &port) in ports.iter().enumerate() {
        let url = custom_url.to_owned();
        probes.spawn(async move { (index, probe_target(port, &url).await) });
    }
    let mut elapsed = vec![None; ports.len()];
    while let Some(result) = probes.join_next().await {
        if let Ok((index, value)) = result {
            elapsed[index] = value;
        }
    }
    elapsed
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
