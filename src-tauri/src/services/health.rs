//! Independent proxy and operating-system path checks. No credentials or destinations are logged.
use crate::domain::errors::{BackendError, Result};
use std::time::Duration;

/// (endpoint, answers with an empty 204). The plain-HTTP probe skips a TLS
/// handshake through the tunnel; captive portals cannot fake an empty 204.
const ENDPOINTS: [(&str, bool); 4] = [
    ("https://api.ipify.org", false),
    ("https://icanhazip.com", false),
    ("https://cp.cloudflare.com/generate_204", true),
    ("http://cp.cloudflare.com/generate_204", true),
];

fn accepted(status: u16, body: &str, empty_expected: bool) -> bool {
    if empty_expected {
        status == 204 && body.trim().is_empty()
    } else {
        status == 200 && body.trim().parse::<std::net::IpAddr>().is_ok()
    }
}

pub async fn check_client(client: &reqwest::Client) -> Result<Option<String>> {
    check(client, true).await
}

pub async fn resolve_ip(proxy_port: u16) -> Result<String> {
    check(&client(Some(proxy_port))?, false)
        .await?
        .ok_or_else(|| BackendError::new("IP_UNAVAILABLE", "Public address is unavailable"))
}

async fn check(client: &reqwest::Client, allow_empty: bool) -> Result<Option<String>> {
    let mut requests = tokio::task::JoinSet::new();
    for (endpoint, empty_expected) in ENDPOINTS {
        if empty_expected && !allow_empty {
            continue;
        }
        let client = client.clone();
        requests.spawn(async move {
            let response = client.get(endpoint).send().await.ok()?;
            let status = response.status().as_u16();
            if response.content_length().is_some_and(|len| len > 256) {
                return None;
            }
            let mut response = response;
            let mut body = Vec::new();
            while let Some(chunk) = response.chunk().await.ok()? {
                if body.len() + chunk.len() > 256 {
                    return None;
                }
                body.extend_from_slice(&chunk);
            }
            let body = std::str::from_utf8(&body).ok()?.trim();
            accepted(status, body, empty_expected).then(|| {
                body.parse::<std::net::IpAddr>()
                    .ok()
                    .map(|ip| ip.to_string())
            })
        });
    }
    while let Some(result) = requests.join_next().await {
        if let Ok(Some(ip)) = result {
            requests.abort_all();
            return Ok(ip);
        }
    }
    Err(BackendError::new(
        "SERVER_UNREACHABLE",
        "Internet connectivity could not be verified",
    ))
}

pub fn client(proxy_port: Option<u16>) -> Result<reqwest::Client> {
    let mut builder = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(5));
    if let Some(port) = proxy_port {
        builder = builder.proxy(
            reqwest::Proxy::all(format!("socks5h://127.0.0.1:{port}"))
                .map_err(|_| BackendError::invalid())?,
        );
    }
    builder.build().map_err(|_| {
        BackendError::new("NETWORK_ERROR", "Could not initialize network verification")
    })
}

pub async fn verify(proxy_port: u16, tun: bool) -> Result<Option<String>> {
    let proxy = client(Some(proxy_port))?;
    if !tun {
        return check_client(&proxy).await;
    }
    #[cfg(windows)]
    crate::platform::network::verify_tunnel_routes()?;
    let system = client(None)?;
    let (proxy_result, system_result) = tokio::join!(check_client(&proxy), check_client(&system));
    let ip = proxy_result?;
    system_result.map_err(|_| {
        BackendError::new(
            "TUN_NETWORK_UNREACHABLE",
            "The proxy works but the system network path does not",
        )
    })?;
    #[cfg(windows)]
    crate::platform::network::verify_tunnel_routes()?;
    Ok(ip)
}

pub fn retryable(code: &str) -> bool {
    matches!(
        code,
        "SERVER_UNREACHABLE"
            | "TUN_NETWORK_UNREACHABLE"
            | "CORE_CRASHED"
            | "CORE_START_FAILED"
            | "TUN_ADAPTER_TIMEOUT"
            | "DNS_UNAVAILABLE"
            | "NETWORK_CHANGED"
    )
}

pub fn allow_recovery(
    attempts: &mut std::collections::VecDeque<std::time::Instant>,
    now: std::time::Instant,
) -> bool {
    while attempts
        .front()
        .is_some_and(|at| now.duration_since(*at) > Duration::from_secs(600))
    {
        attempts.pop_front();
    }
    if attempts.len() >= 5 {
        return false;
    }
    attempts.push_back(now);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rapid_success_then_crash_still_exhausts_recovery_budget() {
        let now = std::time::Instant::now();
        let mut attempts = Default::default();
        for _ in 0..5 {
            assert!(allow_recovery(&mut attempts, now));
        }
        assert!(!allow_recovery(
            &mut attempts,
            now + Duration::from_secs(60)
        ));
        assert!(allow_recovery(
            &mut attempts,
            now + Duration::from_secs(601)
        ));
    }
    #[test]
    fn rejects_captive_portals_redirects_and_malformed_ip() {
        assert!(!accepted(200, "<html>login</html>", false));
        assert!(!accepted(302, "", true));
        assert!(!accepted(200, "", true));
        assert!(accepted(204, "", true));
        assert!(accepted(200, "2001:db8::1\n", false));
        assert!(accepted(200, "192.0.2.1", false));
    }
    #[test]
    fn permanent_errors_and_permission_refusal_do_not_retry() {
        for code in [
            "PERMISSION_DENIED",
            "INVALID_CONFIG",
            "ROUTING_CONFLICT",
            "CANCELLED",
            "STORAGE_FAILED",
        ] {
            assert!(!retryable(code));
        }
        assert!(retryable("SERVER_UNREACHABLE"));
    }
}
