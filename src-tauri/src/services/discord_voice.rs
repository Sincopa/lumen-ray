//! Connect-time check that the selected exit reaches Discord's voice servers.
//!
//! Some servers carry TCP and ordinary UDP but drop Discord voice, and a call
//! then stays on "No Route" while the rest of Discord works. Discord publishes
//! its current voice addresses for its own latency test, and each of them
//! answers an IP discovery packet without a session. The check sends those
//! packets through a local SOCKS inbound, so it measures the VPN exit even
//! after Discord media was moved elsewhere. No addresses are logged.
//!
//! When the selected server drops Discord media, voice first leaves directly,
//! which connects within seconds. Then another server of the same subscription
//! that answers takes Discord media: a screen share on the direct path is cut
//! by Russian ISPs within about a minute, while low-rate voice survives there.
use crate::app::state::AppState;
use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::{atomic::Ordering, Arc},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpStream, UdpSocket},
};

const ENDPOINTS: &str = "https://latency.discord.media/rtc";
const MAX_ENDPOINTS: usize = 6;
const MAX_BODY: usize = 16 * 1024;
/// Voice servers answer discovery on every port of their current range.
const VOICE_PORT_FIRST: u16 = 19294;
const VOICE_PORT_COUNT: u16 = 50;
const ATTEMPTS: usize = 3;
const ATTEMPT_TIMEOUT: Duration = Duration::from_millis(1500);
const DIRECT: &str = "direct";

#[derive(serde::Deserialize)]
struct Region {
    ips: Vec<String>,
}

/// After a verified TUN connection, find the route Discord media should take
/// while `server_id` is the exit, and update the running routing when it changed.
pub fn schedule(state: Arc<AppState>, server_id: String, proxy_port: u16, generation: u64) {
    tauri::async_runtime::spawn(async move {
        // A newer connection may have switched the exit behind this port mid-check.
        let current = || state.connection_generation.load(Ordering::Acquire) == generation;
        let Some(endpoints) = endpoints(proxy_port).await else {
            log::info!("Discord voice check skipped: voice servers were not listed");
            return;
        };
        let Some(reachable) = check(proxy_port, &endpoints).await else {
            log::info!("Discord voice check skipped: the local proxy did not accept UDP");
            return;
        };
        if !current() {
            return;
        }
        if reachable {
            log::info!("Discord voice check: the server carries voice");
            update(&state, &server_id, None, generation).await;
            return;
        }
        log::warn!("Discord voice check: the server drops voice");
        let previous = state
            .repository
            .read()
            .await
            .discord_voice_routes
            .get(&server_id)
            .cloned();
        if previous.is_none() {
            update(&state, &server_id, Some(DIRECT.into()), generation).await;
        }
        #[cfg(windows)]
        let fallback = fallback(
            &state,
            &server_id,
            previous.as_deref(),
            &endpoints,
            &current,
        )
        .await;
        #[cfg(not(windows))]
        let fallback: Option<String> = None;
        if !current() {
            return;
        }
        match &fallback {
            Some(_) => {
                log::info!("Discord voice and streams use another server of the subscription")
            }
            None => log::warn!(
                "No server of the subscription carries Discord voice; voice leaves directly"
            ),
        }
        update(
            &state,
            &server_id,
            Some(fallback.unwrap_or_else(|| DIRECT.into())),
            generation,
        )
        .await;
    });
}

/// Save the route and apply it to the running core without reconnecting.
async fn update(state: &Arc<AppState>, server_id: &str, route: Option<String>, generation: u64) {
    let _transaction = state.updates.lock().await;
    if state.connection_generation.load(Ordering::Acquire) != generation {
        return;
    }
    let change =
        match super::catalog_service::set_discord_voice_route(state, server_id, route).await {
            Ok(change) => change,
            Err(error) => {
                log::warn!("Discord voice route was not saved: {}", error.code);
                return;
            }
        };
    let Some((before, after)) = change else {
        return;
    };
    #[cfg(windows)]
    match super::vpn_service::apply_live_rules(state, server_id, &before, &after).await {
        Ok(crate::engine::LiveRoutingStatus::Applied) => {
            log::info!("Discord voice route applied without reconnecting");
        }
        Ok(_) => log::warn!("Discord voice route is saved and applies on the next connection"),
        Err(error) => log::warn!(
            "Discord voice route is saved but was not applied: {}",
            error.code
        ),
    }
    #[cfg(not(windows))]
    let _ = (before, after);
}

/// The first server of the subscription that answers Discord's voice servers,
/// checked one at a time in a short-lived core. The previous choice goes first,
/// then the order used for automatic server selection.
#[cfg(windows)]
async fn fallback(
    state: &AppState,
    selected: &str,
    previous: Option<&str>,
    endpoints: &[Ipv4Addr],
    current: &(dyn Fn() -> bool + Sync),
) -> Option<String> {
    let candidates: Vec<String> = {
        let repo = state.repository.read().await;
        let subscription = &repo
            .nodes
            .iter()
            .find(|node| node.id == selected)?
            .subscription_id;
        repo.nodes
            .iter()
            .filter(|node| {
                node.subscription_id == *subscription
                    && node.id != selected
                    && node.balancer.is_none()
            })
            .map(|node| node.id.clone())
            .collect()
    };
    let mut ranked: Vec<String> = super::server_quality::rank(
        candidates.clone(),
        selected,
        &*state.server_quality.read().await,
    )
    .into_iter()
    .filter(|id| id != selected)
    .collect();
    if let Some(previous) = previous.filter(|id| candidates.iter().any(|candidate| candidate == id))
    {
        ranked.retain(|id| id != previous);
        ranked.insert(0, previous.to_owned());
    }
    for id in ranked {
        if !current() {
            return None;
        }
        let Some(core) = super::latency_service::isolated_core(state, &id).await else {
            continue;
        };
        let carries = check(core.proxy_port, endpoints).await;
        core.stop().await;
        if carries == Some(true) {
            return Some(id);
        }
    }
    None
}

/// `Some(true)`: a voice server answered through the proxy. `Some(false)`: the
/// proxy accepted the requests and no voice server answered. `None`: the local
/// proxy did not accept a UDP association, so nothing was measured.
async fn check(proxy_port: u16, endpoints: &[Ipv4Addr]) -> Option<bool> {
    let mut probes = tokio::task::JoinSet::new();
    for &ip in endpoints {
        probes.spawn(tokio::time::timeout(
            ATTEMPT_TIMEOUT * ATTEMPTS as u32 + Duration::from_secs(3),
            discover(proxy_port, ip),
        ));
    }
    let mut measured = false;
    while let Some(result) = probes.join_next().await {
        match result {
            Ok(Ok(Some(true))) => {
                probes.abort_all();
                return Some(true);
            }
            Ok(Ok(Some(false))) => measured = true,
            _ => {}
        }
    }
    measured.then_some(false)
}

/// Up to `MAX_ENDPOINTS` IPv4 voice servers, one region at a time, fetched
/// through the VPN: Discord's own hosts are blocked on many direct paths.
async fn endpoints(proxy_port: u16) -> Option<Vec<Ipv4Addr>> {
    let client = super::health::client(Some(proxy_port)).ok()?;
    let mut response = client.get(ENDPOINTS).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.ok()? {
        if body.len() + chunk.len() > MAX_BODY {
            return None;
        }
        body.extend_from_slice(&chunk);
    }
    let regions: Vec<Region> = serde_json::from_slice(&body).ok()?;
    let endpoints = select_endpoints(&regions);
    (!endpoints.is_empty()).then_some(endpoints)
}

/// Spread the checks across regions: the first address of every region, then
/// the second, and so on. Only public IPv4 addresses are used.
fn select_endpoints(regions: &[Region]) -> Vec<Ipv4Addr> {
    let lists: Vec<Vec<Ipv4Addr>> = regions
        .iter()
        .map(|region| {
            region
                .ips
                .iter()
                .filter_map(|ip| ip.parse::<Ipv4Addr>().ok())
                .filter(|ip| {
                    !(ip.is_private()
                        || ip.is_loopback()
                        || ip.is_link_local()
                        || ip.is_unspecified()
                        || ip.is_broadcast()
                        || ip.is_multicast())
                })
                .collect()
        })
        .collect();
    let mut selected = Vec::new();
    for round in 0.. {
        let mut any = false;
        for list in &lists {
            if let Some(ip) = list.get(round) {
                any = true;
                if !selected.contains(ip) {
                    selected.push(*ip);
                }
                if selected.len() == MAX_ENDPOINTS {
                    return selected;
                }
            }
        }
        if !any {
            break;
        }
    }
    selected
}

/// Discord IP discovery: type 1, length 70, SSRC, then 64 address and 2 port bytes.
fn discovery_packet(ssrc: u32) -> [u8; 74] {
    let mut packet = [0u8; 74];
    packet[..2].copy_from_slice(&1u16.to_be_bytes());
    packet[2..4].copy_from_slice(&70u16.to_be_bytes());
    packet[4..8].copy_from_slice(&ssrc.to_be_bytes());
    packet
}

/// The answer is type 2, length 70, carrying the address the server saw.
fn is_discovery_reply(payload: &[u8]) -> bool {
    payload.len() >= 74 && payload[..2] == [0, 2] && payload[2..4] == [0, 70]
}

/// One SOCKS5 UDP association per server: a single association pins its later
/// packets to the first destination. `Some(false)` means the proxy accepted
/// the packets and the server stayed silent.
async fn discover(proxy_port: u16, ip: Ipv4Addr) -> Option<bool> {
    let mut control = TcpStream::connect((Ipv4Addr::LOCALHOST, proxy_port))
        .await
        .ok()?;
    control.write_all(&[5, 1, 0]).await.ok()?;
    let mut greeting = [0u8; 2];
    control.read_exact(&mut greeting).await.ok()?;
    if greeting != [5, 0] {
        return None;
    }
    control
        .write_all(&[5, 3, 0, 1, 0, 0, 0, 0, 0, 0])
        .await
        .ok()?;
    let mut reply = [0u8; 4];
    control.read_exact(&mut reply).await.ok()?;
    if reply[..2] != [5, 0] {
        return None;
    }
    let relay = match reply[3] {
        1 => {
            let mut address = [0u8; 6];
            control.read_exact(&mut address).await.ok()?;
            let host = Ipv4Addr::new(address[0], address[1], address[2], address[3]);
            let port = u16::from_be_bytes([address[4], address[5]]);
            let host = if host.is_unspecified() {
                Ipv4Addr::LOCALHOST
            } else {
                host
            };
            SocketAddr::from((host, port))
        }
        _ => return None,
    };
    let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).await.ok()?;
    let random = *uuid::Uuid::new_v4().as_bytes();
    let ssrc = u32::from_be_bytes([random[0], random[1], random[2], random[3]]);
    let port = VOICE_PORT_FIRST + u16::from_be_bytes([random[4], random[5]]) % VOICE_PORT_COUNT;
    let mut datagram = Vec::with_capacity(84);
    datagram.extend_from_slice(&[0, 0, 0, 1]);
    datagram.extend_from_slice(&ip.octets());
    datagram.extend_from_slice(&port.to_be_bytes());
    datagram.extend_from_slice(&discovery_packet(ssrc));
    let mut buffer = [0u8; 512];
    for _ in 0..ATTEMPTS {
        socket.send_to(&datagram, relay).await.ok()?;
        let deadline = tokio::time::Instant::now() + ATTEMPT_TIMEOUT;
        while let Ok(Ok((length, _))) =
            tokio::time::timeout_at(deadline, socket.recv_from(&mut buffer)).await
        {
            // SOCKS5 UDP header for an IPv4 source: 10 bytes.
            if length > 10 && buffer[3] == 1 && is_discovery_reply(&buffer[10..length]) {
                return Some(true);
            }
        }
    }
    drop(control);
    Some(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoints_alternate_regions_and_skip_unusable_addresses() {
        let regions: Vec<Region> = serde_json::from_str(
            r#"[{"region":"helsinki","ips":["104.29.155.201","104.29.158.3","10.0.0.1"]},
                {"region":"stockholm","ips":["::1","104.29.136.181"]},
                {"region":"empty","ips":[]}]"#,
        )
        .unwrap();
        let selected: Vec<String> = select_endpoints(&regions)
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(
            selected,
            ["104.29.155.201", "104.29.136.181", "104.29.158.3"]
        );
    }

    #[test]
    fn discovery_packet_matches_discord_format() {
        let packet = discovery_packet(0x0102_0304);
        assert_eq!(packet.len(), 74);
        assert_eq!(&packet[..8], &[0, 1, 0, 70, 1, 2, 3, 4]);
        let mut reply = packet;
        reply[1] = 2;
        assert!(is_discovery_reply(&reply));
        assert!(!is_discovery_reply(&packet));
        assert!(!is_discovery_reply(&reply[..20]));
    }
}
