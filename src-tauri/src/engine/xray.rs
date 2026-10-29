//! Xray process, helper, standby and live-routing implementation.
use super::{BackendFuture, BackendSession, CoreBackend};
use crate::{
    app::state::AppState,
    domain::errors::{BackendError, Result},
    storage::repository::Repository,
};
use sha2::{Digest, Sha256};
use std::{
    process::Stdio,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{io::AsyncWriteExt, process::Command};
#[derive(Default)]
pub struct XraySession {
    pub is_tun: bool,
    #[cfg(any(windows, target_os = "macos"))]
    pub child: Option<crate::platform::process::CoreProcess>,
    #[cfg(not(any(windows, target_os = "macos")))]
    pub child: Option<tokio::process::Child>,
    #[cfg(windows)]
    pub helper: Option<crate::platform::helper::Helper>,
    pub proxy_port: u16,
    pub metrics_port: u16,
    pub started: Option<Instant>,
    /// Server the running core carries; a failed switch restores the snapshot only for it.
    #[cfg(any(windows, target_os = "macos", target_os = "android"))]
    pub prepared_server_id: Option<String>,
    #[cfg(any(windows, target_os = "macos"))]
    pub prepared_config_hash: Option<[u8; 32]>,
    #[cfg(any(windows, target_os = "macos"))]
    pub prepared_core_version: Option<String>,
    #[cfg(any(windows, target_os = "macos"))]
    pub prepared_geo_hash: Option<[u8; 32]>,
    #[cfg(any(windows, target_os = "macos"))]
    pub base_server_id: Option<String>,
    #[cfg(any(windows, target_os = "macos"))]
    base_outbound_hash: Option<[u8; 32]>,
    #[cfg(any(windows, target_os = "macos"))]
    pub active_proxy_tag: Option<String>,
    #[cfg(any(windows, target_os = "macos"))]
    pub live_outbound_tags: Vec<(String, String, [u8; 32])>,
    #[cfg(any(windows, target_os = "macos"))]
    pub pending_live_switch: Option<LiveSwitchRollback>,
    /// Repository the running core's routing currently reflects (sources removed).
    #[cfg(any(windows, target_os = "macos"))]
    prepared_repository: Option<Arc<Repository>>,
    /// Repository the running core was started with; it owns the loaded outbounds.
    #[cfg(any(windows, target_os = "macos"))]
    base_repository: Option<Arc<Repository>>,
}

#[cfg(any(windows, target_os = "macos"))]
pub struct LiveSwitchRollback {
    previous_server_id: String,
    previous_config_hash: [u8; 32],
    previous_proxy_tag: String,
    previous_repository: Arc<Repository>,
    routing_patch: serde_json::Value,
}

#[cfg(any(windows, target_os = "macos"))]
fn stripped(repo: &Repository) -> Arc<Repository> {
    let mut repo = repo.clone();
    repo.sources.clear();
    Arc::new(repo)
}

/// The control API is reachable while the core process runs: an active or
/// prepared TUN helper, or the system-proxy core of a connected session.
#[cfg(any(windows, target_os = "macos"))]
fn api_available(session: &XraySession) -> bool {
    #[cfg(windows)]
    if session.is_tun {
        return session
            .helper
            .as_ref()
            .is_some_and(|helper| !helper.exited());
    }
    // macOS has no TUN helper yet, so a TUN session never has a live API.
    !session.is_tun
        && session.started.is_some()
        && session
            .child
            .as_ref()
            .is_some_and(|child| matches!(child.try_wait(), Ok(None)))
}
/// Android always tunnels through VpnService; there is no system proxy mode.
fn tun_mode(repo: &Repository) -> bool {
    cfg!(target_os = "android") || repo.settings.network_mode == "tun"
}
/// TUN on macOS needs a privileged helper that does not exist yet.
fn supported(repo: &Repository) -> Result<()> {
    if cfg!(target_os = "macos") && repo.settings.network_mode == "tun" {
        return Err(BackendError::new(
            "PLATFORM_UNSUPPORTED",
            "TUN mode is not available on macOS yet",
        ));
    }
    Ok(())
}
/// Go runtime tuning for every xray child. A large geosite category (antifilter has
/// ~620k domains) keeps ~120 MB live; a GOMEMLIMIT below that made the GC run
/// non-stop (28 s of CPU to start, constant CPU under traffic) without saving memory.
/// GOGC=50 keeps traffic garbage close to the live set; the limit is only a backstop.
pub const GO_RUNTIME_ENV: [(&str, &str); 3] = [
    ("GOGC", "50"),
    ("GOMEMLIMIT", "384MiB"),
    ("GODEBUG", "madvdontneed=1"),
];
pub fn command(core: &std::path::Path) -> Command {
    let mut command = Command::new(core);
    if let Some(parent) = core.parent() {
        command.current_dir(parent);
        command.env(
            "XRAY_LOCATION_ASSET",
            crate::xray::geo::assets_for_core(core),
        );
    } else {
        command.current_dir(core);
    }
    command.envs(GO_RUNTIME_ENV);
    command.kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    command
}
pub async fn validate(core: &std::path::Path, config: &[u8]) -> Result<()> {
    let mut child = command(core)
        .args(["run", "-test", "-config", "stdin:"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| {
            BackendError::new("CORE_START_FAILED", "Could not run Xray config validation")
        })?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(config).await.map_err(|_| {
            BackendError::new("INVALID_CONFIG", "Could not pass configuration to Xray")
        })?;
    }
    let output = tokio::time::timeout(Duration::from_secs(10), child.wait_with_output())
        .await
        .map_err(|_| BackendError::new("INVALID_CONFIG", "Xray validation timed out"))?
        .map_err(|_| BackendError::new("INVALID_CONFIG", "Xray validation failed"))?;
    if !output.status.success() {
        let output = format!(
            "{} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let failure = output
            .lines()
            .find(|line| line.contains("Failed to start"))
            .unwrap_or(&output);
        return Err(rejection(failure));
    }
    Ok(())
}
/// Maps an Xray load failure to a code the UI explains, without echoing
/// server parameters from the message.
fn rejection(failure: &str) -> BackendError {
    if failure.contains("failed to open geoip.dat")
        || failure.contains("failed to open geosite.dat")
        || failure.contains("failed to check code")
        || failure.contains("illegal ip rule: geoip:")
        || failure.contains("illegal domain rule: geosite:")
    {
        return BackendError::new(
            "GEO_DATA_MISSING",
            "GeoIP/GeoSite database files are missing or corrupted. Please update databases in Settings.",
        );
    }
    let category = [
        "fingerprint",
        "password",
        "shortId",
        "serverName",
        "flow",
        "ALPN",
        "alpn",
        "encryption",
        "security",
        "Vision",
        "REALITY",
        "routing",
        "Host",
        "headers",
        "removed",
        "WebSocket",
        "XHTTP",
    ]
    .into_iter()
    .find(|s| failure.contains(s))
    .unwrap_or("configuration");
    BackendError::new(
        "INVALID_CONFIG",
        format!("Xray rejected {category} parameters"),
    )
}
pub(crate) fn ports() -> Result<(u16, u16)> {
    let error = || BackendError::new("CORE_START_FAILED", "No local ports available");
    // Hold reservations together: Windows may reuse a just-released ephemeral port.
    let proxy = std::net::TcpListener::bind("127.0.0.1:0").map_err(|_| error())?;
    for _ in 0..32 {
        let metrics = std::net::TcpListener::bind("127.0.0.1:0").map_err(|_| error())?;
        let port = metrics.local_addr().map_err(|_| error())?.port();
        if let (Some(http), Some(api)) = (port.checked_add(1), port.checked_add(2)) {
            if let (Ok(_http), Ok(_api)) = (
                std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, http)),
                std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, api)),
            ) {
                return Ok((proxy.local_addr().map_err(|_| error())?.port(), port));
            }
        }
    }
    Err(error())
}
#[cfg(any(windows, target_os = "macos"))]
fn configuration_fingerprint(
    repo: &crate::storage::repository::Repository,
    selected: &str,
    tun: bool,
) -> Result<[u8; 32]> {
    let config = crate::xray::config::build(repo, selected, 10808, 10809, tun)?;
    let mut config = serde_json::to_value(config)
        .map_err(|_| BackendError::new("INVALID_CONFIG", "Could not serialize Xray config"))?;
    // The live physical resolver can change while TUN is active. It is not a
    // repository change and must not invalidate routing-only updates or standby.
    if repo.settings.dns_server == "system" && !repo.settings.dns_policy.enabled {
        if let Some(inbounds) = config["inbounds"].as_array_mut() {
            for inbound in inbounds {
                if inbound["protocol"] == "tun" {
                    inbound["settings"]["dns"] = serde_json::json!(["system"]);
                }
            }
        }
        if config["dns"].is_object() {
            config["dns"]["servers"] = serde_json::json!(["system"]);
        }
    }
    let mut hasher = Sha256::new();
    serde_json::to_writer(&mut hasher, &config)
        .map_err(|_| BackendError::new("INVALID_CONFIG", "Could not serialize Xray config"))?;
    Ok(hasher.finalize().into())
}

#[cfg(any(windows, target_os = "macos"))]
fn geo_fingerprint(data: &std::path::Path) -> Option<[u8; 32]> {
    std::fs::read(crate::xray::geo::metadata_path(data))
        .ok()
        .map(|bytes| Sha256::digest(bytes).into())
}

#[cfg(any(windows, target_os = "macos"))]
fn assets_match(session: &XraySession, version: &str, geo: Option<[u8; 32]>) -> bool {
    session.prepared_core_version.as_deref() == Some(version) && session.prepared_geo_hash == geo
}

/// A live elevated helper is never replaced by another UAC prompt.
#[cfg(windows)]
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum HelperPlan {
    Reuse,
    Reload,
    Launch,
}

#[cfg(windows)]
pub(crate) fn helper_plan(alive: bool, config_matches: bool) -> HelperPlan {
    if !alive {
        HelperPlan::Launch
    } else if config_matches {
        HelperPlan::Reuse
    } else {
        HelperPlan::Reload
    }
}

#[cfg(windows)]
#[allow(clippy::too_many_arguments)]
fn note_prepared(
    session: &mut XraySession,
    selected: &str,
    hash: [u8; 32],
    version: &str,
    geo: Option<[u8; 32]>,
    outbound_hash: [u8; 32],
    repository: Arc<Repository>,
    proxy_port: u16,
    metrics_port: u16,
) {
    session.prepared_server_id = Some(selected.into());
    session.prepared_config_hash = Some(hash);
    session.prepared_core_version = Some(version.into());
    session.prepared_geo_hash = geo;
    session.base_server_id = Some(selected.into());
    session.base_outbound_hash = Some(outbound_hash);
    session.base_repository = Some(repository.clone());
    session.prepared_repository = Some(repository);
    session.active_proxy_tag = Some("proxy".into());
    session.live_outbound_tags.clear();
    session.pending_live_switch = None;
    session.proxy_port = proxy_port;
    session.metrics_port = metrics_port;
    session.is_tun = true;
}

#[cfg(any(windows, target_os = "macos"))]
fn outbound_fingerprint(repo: &Repository, selected: &str) -> Result<[u8; 32]> {
    let node = repo
        .nodes
        .iter()
        .find(|node| node.id == selected)
        .ok_or_else(BackendError::invalid)?;
    let outbound = crate::xray::config::outbound(node, "proxy")?;
    let mut hasher = Sha256::new();
    serde_json::to_writer(&mut hasher, &outbound).map_err(|_| BackendError::invalid())?;
    Ok(hasher.finalize().into())
}

#[cfg(any(windows, target_os = "macos"))]
fn cached_outbound_matches(session: &XraySession, selected: &str, hash: [u8; 32]) -> bool {
    if session.base_server_id.as_deref() == Some(selected) {
        return session.base_outbound_hash == Some(hash);
    }
    session
        .live_outbound_tags
        .iter()
        .find(|(id, _, _)| id == selected)
        .is_none_or(|(_, _, cached)| *cached == hash)
}

/// Apply a routing-only catalog change to the running core (active or prepared
/// TUN, or a connected system proxy) without restarting it.
/// `NeedsConnect` is only a network-mode change or a core that is not running.
/// Any other miss leaves the tunnel up (`KeepTunnel`).
#[cfg(any(windows, target_os = "macos"))]
pub async fn apply_live_rules(
    state: &Arc<AppState>,
    selected: &str,
    before: Option<&Repository>,
    after: &Repository,
    session: &mut XraySession,
) -> Result<crate::engine::LiveRoutingStatus> {
    let tun = session.is_tun;
    if (after.settings.network_mode == "tun") != tun {
        log::info!("engine=xray routing update needs a new connection: network mode changed");
        return Ok(crate::engine::LiveRoutingStatus::NeedsConnect);
    }
    if !api_available(session) {
        log::info!("engine=xray routing update skipped: core is not running");
        return Ok(crate::engine::LiveRoutingStatus::NeedsConnect);
    }
    if session.prepared_server_id.as_deref() != Some(selected) {
        log::warn!("engine=xray routing update kept the tunnel: active server changed");
        return Ok(crate::engine::LiveRoutingStatus::KeepTunnel);
    }
    if session.pending_live_switch.is_some() {
        return Ok(crate::engine::LiveRoutingStatus::KeepTunnel);
    }
    let after_hash = configuration_fingerprint(after, selected, tun)?;
    if session.prepared_config_hash == Some(after_hash) {
        return Ok(crate::engine::LiveRoutingStatus::Applied);
    }
    // Patch from what the core actually runs; a caller snapshot may be stale
    // after an earlier failed update.
    let current = match (session.prepared_repository.clone(), before) {
        (Some(current), _) => current,
        (None, Some(before))
            if session.prepared_config_hash
                == Some(configuration_fingerprint(before, selected, tun)?) =>
        {
            stripped(before)
        }
        _ => {
            log::warn!("engine=xray routing update kept the tunnel: configuration changed");
            return Ok(crate::engine::LiveRoutingStatus::KeepTunnel);
        }
    };
    let Some(mut plan) = crate::xray::config::live_routing_plan(&current, after, selected, tun)?
    else {
        log::info!("engine=xray routing update kept the tunnel");
        return Ok(crate::engine::LiveRoutingStatus::KeepTunnel);
    };
    if let Some(tag) = session.active_proxy_tag.as_deref() {
        replace_primary_proxy_tag(&mut plan.patch, tag);
    }
    let core = state.core.read().await.clone();
    let applied_at = Instant::now();
    let api_port = session.metrics_port + 2;
    for outbound in &plan.added_outbounds {
        if let Err(error) = run_live_outbound_add(&core, api_port, outbound).await {
            log::warn!(
                "engine=xray routing outbound add failed: {} ({})",
                error.code,
                error.message
            );
            return Err(error);
        }
    }
    if let Err(error) = run_live_routing_patch(&core, api_port, &plan.patch).await {
        log::warn!(
            "engine=xray routing update failed: {} ({})",
            error.code,
            error.message
        );
        return Err(error);
    }
    session.prepared_config_hash = Some(after_hash);
    session.prepared_repository = Some(stripped(after));
    log::info!(
        "engine=xray routing update applied {} ms",
        applied_at.elapsed().as_millis()
    );
    Ok(crate::engine::LiveRoutingStatus::Applied)
}

/// Bring a running core to the requested server and routing through its API.
/// Errors are reported as `false`: a restart remains the supported fallback.
#[cfg(any(windows, target_os = "macos"))]
async fn update_running_core(
    state: &Arc<AppState>,
    repo: &Repository,
    selected: &str,
    config_hash: [u8; 32],
    session: &mut XraySession,
) -> bool {
    let result = if session.prepared_server_id.as_deref() == Some(selected) {
        if session.prepared_config_hash == Some(config_hash) {
            return true;
        }
        apply_live_rules(state, selected, None, repo, session)
            .await
            .map(|status| status == crate::engine::LiveRoutingStatus::Applied)
    } else if session.started.is_some() {
        // A routing patch steers only new connections. Open ones (browser
        // keep-alive, QUIC, voice) would stay on the old server until they
        // close, so a server change on a carrying core restarts it. A standby
        // core carries nothing and still switches through its API.
        return false;
    } else {
        switch_server_live(state, repo, selected, config_hash, session).await
    };
    result.unwrap_or_else(|error| {
        log::warn!("engine=xray live update needs restart: {}", error.code);
        false
    })
}

#[cfg(any(windows, target_os = "macos"))]
fn replace_primary_proxy_tag(value: &mut serde_json::Value, tag: &str) {
    match value {
        serde_json::Value::Object(fields) => {
            if fields
                .get("outboundTag")
                .and_then(serde_json::Value::as_str)
                == Some("proxy")
            {
                fields.insert("outboundTag".into(), serde_json::Value::String(tag.into()));
            }
            for child in fields.values_mut() {
                replace_primary_proxy_tag(child, tag);
            }
        }
        serde_json::Value::Array(items) => {
            for child in items {
                replace_primary_proxy_tag(child, tag);
            }
        }
        _ => {}
    }
}

#[cfg(any(windows, target_os = "macos"))]
struct LiveSwitchPlan {
    outbound: serde_json::Value,
    forward_patch: serde_json::Value,
    rollback_patch: serde_json::Value,
}

#[cfg(any(windows, target_os = "macos"))]
#[allow(clippy::too_many_arguments)]
fn live_switch_plan(
    base_repo: &Repository,
    previous_repo: &Repository,
    repo: &Repository,
    (base_server, previous_server, next_server): (&str, &str, &str),
    previous_tag: &str,
    next_tag: &str,
    ports: (u16, u16),
    tun: bool,
) -> Result<Option<LiveSwitchPlan>> {
    if [
        (base_repo, base_server),
        (previous_repo, previous_server),
        (repo, next_server),
    ]
    .into_iter()
    .any(|(repo, id)| {
        repo.nodes
            .iter()
            .find(|node| node.id == id)
            .is_none_or(|node| node.balancer.is_some())
    }) {
        return Ok(None);
    }
    let config = |repo, selected| {
        crate::xray::config::build(repo, selected, ports.0, ports.1, tun)
            .and_then(|config| serde_json::to_value(config).map_err(|_| BackendError::invalid()))
    };
    // `base` is what the core loaded at start; `before` is its current routing.
    let base = config(base_repo, base_server)?;
    let before = config(previous_repo, previous_server)?;
    let after = config(repo, next_server)?;
    if ["inbounds", "dns", "observatory", "burstObservatory"]
        .into_iter()
        .any(|key| base[key] != after[key])
        || ["domainStrategy", "balancers"]
            .into_iter()
            .any(|key| base["routing"][key] != after["routing"][key])
    {
        return Ok(None);
    }
    let Some(base_outbounds) = base["outbounds"].as_array() else {
        return Ok(None);
    };
    let Some(desired_outbounds) = after["outbounds"].as_array() else {
        return Ok(None);
    };
    if desired_outbounds
        .iter()
        .filter(|outbound| outbound["tag"] != "proxy")
        .any(|outbound| !base_outbounds.contains(outbound))
    {
        return Ok(None);
    }
    let Some(mut outbound) = desired_outbounds
        .iter()
        .find(|outbound| outbound["tag"] == "proxy")
        .cloned()
    else {
        return Ok(None);
    };
    outbound["tag"] = serde_json::Value::String(next_tag.into());
    let mut forward_patch = serde_json::json!({"routing":after["routing"]});
    replace_primary_proxy_tag(&mut forward_patch, next_tag);
    let mut rollback_patch = serde_json::json!({"routing":before["routing"]});
    replace_primary_proxy_tag(&mut rollback_patch, previous_tag);
    Ok(Some(LiveSwitchPlan {
        outbound,
        forward_patch,
        rollback_patch,
    }))
}

#[cfg(all(test, any(windows, target_os = "macos")))]
mod live_switch_tests {
    use super::*;

    #[test]
    fn active_tun_reuse_requires_current_geo_and_core() {
        let directory = tempfile::tempdir().unwrap();
        let metadata = crate::xray::geo::metadata_path(directory.path());
        std::fs::create_dir_all(metadata.parent().unwrap()).unwrap();
        std::fs::write(&metadata, b"old-generation").unwrap();
        let node = crate::subscription::uri::parse("socks5://example.com:1080", "sub").unwrap();
        let mut repo = Repository {
            nodes: vec![node.clone()],
            ..Default::default()
        };
        repo.settings.dns_server = "1.1.1.1".into();
        let hash = configuration_fingerprint(&repo, &node.id, true).unwrap();
        let session = XraySession {
            prepared_config_hash: Some(hash),
            prepared_core_version: Some("test-version".into()),
            prepared_geo_hash: geo_fingerprint(directory.path()),
            ..Default::default()
        };
        assert!(assets_match(
            &session,
            "test-version",
            geo_fingerprint(directory.path())
        ));
        std::fs::write(&metadata, b"new-generation").unwrap();
        // Updating files cannot invalidate the repository/configuration hash.
        assert_eq!(
            session.prepared_config_hash,
            Some(configuration_fingerprint(&repo, &node.id, true).unwrap())
        );
        assert!(!assets_match(
            &session,
            "test-version",
            geo_fingerprint(directory.path())
        ));
        assert!(!assets_match(
            &session,
            "next-version",
            session.prepared_geo_hash
        ));
    }

    #[test]
    fn credential_rotation_invalidates_base_and_dynamic_outbound_cache() {
        let node =
            crate::subscription::uri::parse("trojan://old-password@example.com:443#Server", "sub")
                .unwrap();
        let mut repo = Repository {
            nodes: vec![node.clone()],
            ..Default::default()
        };
        let hash = outbound_fingerprint(&repo, &node.id).unwrap();
        let mut session = XraySession {
            base_server_id: Some(node.id.clone()),
            base_outbound_hash: Some(hash),
            ..Default::default()
        };
        assert!(cached_outbound_matches(&session, &node.id, hash));
        repo.nodes[0].credential = "new-password".into(); // Refresh preserves the ID.
        let rotated = outbound_fingerprint(&repo, &node.id).unwrap();
        assert!(!cached_outbound_matches(&session, &node.id, rotated));
        session.base_server_id = Some("other".into());
        session
            .live_outbound_tags
            .push((node.id.clone(), "live-tag".into(), hash));
        assert!(cached_outbound_matches(&session, &node.id, hash));
        assert!(!cached_outbound_matches(&session, &node.id, rotated));
    }

    #[cfg(windows)]
    #[test]
    fn live_helper_is_reloaded_instead_of_launched() {
        assert_eq!(helper_plan(true, true), HelperPlan::Reuse);
        assert_eq!(helper_plan(true, false), HelperPlan::Reload);
        assert_eq!(helper_plan(false, true), HelperPlan::Launch);
        assert_eq!(helper_plan(false, false), HelperPlan::Launch);
    }

    #[test]
    fn switches_primary_outbound_without_changing_tun_and_keeps_rollback_route() {
        let first = crate::subscription::uri::parse(
            "vless://00000000-0000-4000-8000-000000000001@a.example:443?security=tls#A",
            "sub",
        )
        .unwrap();
        let second = crate::subscription::uri::parse(
            "vless://00000000-0000-4000-8000-000000000002@b.example:443?security=tls#B",
            "sub",
        )
        .unwrap();
        let mut repo = Repository {
            nodes: vec![first.clone(), second.clone()],
            routing_mode: "global".into(),
            ..Default::default()
        };
        repo.settings.dns_server = "1.1.1.1".into();
        let plan = live_switch_plan(
            &repo,
            &repo,
            &repo,
            (&first.id, &first.id, &second.id),
            "lumen-live-old",
            "lumen-live-next",
            (10808, 10809),
            true,
        )
        .unwrap()
        .unwrap();
        assert_eq!(plan.outbound["tag"], "lumen-live-next");
        assert_eq!(
            plan.forward_patch["routing"]["rules"]
                .as_array()
                .unwrap()
                .last()
                .unwrap()["outboundTag"],
            "lumen-live-next"
        );
        assert_eq!(
            plan.rollback_patch["routing"]["rules"]
                .as_array()
                .unwrap()
                .last()
                .unwrap()["outboundTag"],
            "lumen-live-old"
        );
        let old_config = serde_json::to_value(
            crate::xray::config::build(&repo, &first.id, 10808, 10809, true).unwrap(),
        )
        .unwrap();
        let new_config = serde_json::to_value(
            crate::xray::config::build(&repo, &second.id, 10808, 10809, true).unwrap(),
        )
        .unwrap();
        assert_eq!(old_config["inbounds"], new_config["inbounds"]);
        assert_eq!(old_config["dns"], new_config["dns"]);
    }

    #[tokio::test]
    async fn bundled_xray_accepts_live_outbound_and_route_commands() {
        let directory = tempfile::tempdir().unwrap();
        let runtime = crate::runtime::RuntimePaths::new(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("runtime"),
        );
        let core = crate::xray::bundle::prepare(&runtime, directory.path()).unwrap();
        let (proxy_port, metrics_port) = ports().unwrap();
        let config = serde_json::json!({
            "log":{"loglevel":"warning"},
            "api":{"tag":"lumen-api","listen":format!("127.0.0.1:{}", metrics_port + 2),"services":["RoutingService","HandlerService"]},
            "inbounds":[{"tag":"probe","listen":"127.0.0.1","port":proxy_port,"protocol":"socks","settings":{"auth":"noauth"}}],
            "outbounds":[{"tag":"proxy","protocol":"blackhole"}],
            "routing":{"rules":[{"type":"field","inboundTag":["probe"],"outboundTag":"proxy"}]}
        });
        let bytes = serde_json::to_vec(&config).unwrap();
        validate(&core, &bytes).await.unwrap();
        let mut session = XraySession::default();
        start(&core, &mut session, &bytes, proxy_port, metrics_port)
            .await
            .unwrap();
        let node = crate::subscription::uri::parse(
            "vless://00000000-0000-4000-8000-000000000001@example.com:443?security=tls#Local-Test",
            "local-test",
        )
        .unwrap();
        let outbound = crate::xray::config::outbound(&node, "lumen-live-test").unwrap();
        let result = async {
            run_live_outbound_add(&core, metrics_port + 2, &outbound).await?;
            run_live_routing_patch(
                &core,
                metrics_port + 2,
                &serde_json::json!({"routing":{"rules":[{"type":"field","inboundTag":["probe"],"outboundTag":"lumen-live-test"}]}}),
            )
            .await
        }
        .await;
        stop(&mut session).await.unwrap();
        result.unwrap();
    }

    #[tokio::test]
    async fn system_proxy_core_takes_rules_live_and_restarts_for_servers() {
        use crate::storage::repository::{Rule, RuleMatcher};
        let directory = tempfile::tempdir().unwrap();
        let runtime = crate::runtime::RuntimePaths::new(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("runtime"),
        );
        let state = AppState::open(directory.path().into(), runtime).unwrap();
        let first = crate::subscription::uri::parse(
            "vless://00000000-0000-4000-8000-000000000001@a.example:443?security=tls#A",
            "sub",
        )
        .unwrap();
        let second = crate::subscription::uri::parse(
            "vless://00000000-0000-4000-8000-000000000002@b.example:443?security=tls#B",
            "sub",
        )
        .unwrap();
        let mut global = Repository {
            nodes: vec![first.clone(), second.clone()],
            routing_mode: "global".into(),
            ..Default::default()
        };
        global.settings.network_mode = "proxy".into();
        global.settings.dns_server = "1.1.1.1".into();
        let mut rules = global.clone();
        rules.routing_mode = "rules".into();
        rules.rules.push(Rule {
            id: "site".into(),
            name: "example.org".into(),
            matcher: RuleMatcher::Domain,
            server_id: "subscription".into(),
            enabled: true,
            preset_id: None,
            is_domain: true,
        });
        let mut session = BackendSession::default();
        let result = async {
            XrayBackend
                .connect(&state, &global, &first.id, &mut session)
                .await?;
            let started = session.started;
            assert!(started.is_some());
            // All -> Rules with a new rule, then a server switch, then back to All.
            assert_eq!(
                XrayBackend
                    .update_routing(&state, &first.id, &global, &rules, &mut session)
                    .await?,
                crate::engine::LiveRoutingStatus::Applied
            );
            // A server change restarts the core so open connections move too.
            XrayBackend
                .connect(&state, &rules, &second.id, &mut session)
                .await?;
            let s = &mut session;
            assert_eq!(s.prepared_server_id.as_deref(), Some(second.id.as_str()));
            assert!(s.pending_live_switch.is_none());
            assert_ne!(s.started, started);
            let started = s.started;
            let mut toggled = rules.clone();
            toggled.rules[0].enabled = false;
            assert_eq!(
                XrayBackend
                    .update_routing(&state, &second.id, &rules, &toggled, &mut session)
                    .await?,
                crate::engine::LiveRoutingStatus::Applied
            );
            let s = &mut session;
            assert_eq!(
                apply_live_rules(&state, &second.id, None, &global, s).await?,
                crate::engine::LiveRoutingStatus::Applied
            );
            // Rule and mode changes keep the process serving the second server.
            assert_eq!(s.started, started);
            Ok::<_, BackendError>(())
        }
        .await;
        stop(&mut session).await.unwrap();
        result.unwrap();
    }
}

#[cfg(any(windows, target_os = "macos"))]
async fn switch_server_live(
    state: &Arc<AppState>,
    repo: &Repository,
    selected: &str,
    config_hash: [u8; 32],
    session: &mut XraySession,
) -> Result<bool> {
    let tun = session.is_tun;
    if (repo.settings.network_mode == "tun") != tun
        || !api_available(session)
        || session.pending_live_switch.is_some()
        || session.live_outbound_tags.len() >= 64
    {
        return Ok(false);
    }
    let (Some(base), Some(previous)) = (
        session.base_server_id.clone(),
        session.prepared_server_id.clone(),
    ) else {
        return Ok(false);
    };
    let (Some(base_repo), Some(previous_repo)) = (
        session.base_repository.clone(),
        session.prepared_repository.clone(),
    ) else {
        return Ok(false);
    };
    if previous == selected {
        return Ok(false);
    }
    let previous_tag = session
        .active_proxy_tag
        .clone()
        .unwrap_or_else(|| "proxy".into());
    let Some(previous_config_hash) = session.prepared_config_hash else {
        return Ok(false);
    };
    let outbound_hash = outbound_fingerprint(repo, selected)?;
    if !cached_outbound_matches(session, selected, outbound_hash) {
        // Subscription refresh can retain the ID while rotating credentials.
        // Restart rather than route to an obsolete cached outbound.
        return Ok(false);
    }
    let existing_tag = session
        .live_outbound_tags
        .iter()
        .find(|(server_id, _, _)| server_id == selected)
        .map(|(_, tag, _)| tag.clone());
    let next_tag = if selected == base {
        "proxy".to_owned()
    } else {
        existing_tag
            .clone()
            .unwrap_or_else(|| format!("lumen-live-{}", uuid::Uuid::new_v4().simple()))
    };
    let add_outbound = selected != base && existing_tag.is_none();
    let Some(plan) = live_switch_plan(
        &base_repo,
        &previous_repo,
        repo,
        (&base, &previous, selected),
        &previous_tag,
        &next_tag,
        (session.proxy_port, session.metrics_port),
        tun,
    )?
    else {
        return Ok(false);
    };
    let core = state.core.read().await.clone();
    let switched_at = Instant::now();
    if add_outbound {
        if let Err(error) =
            run_live_outbound_add(&core, session.metrics_port + 2, &plan.outbound).await
        {
            log::warn!(
                "engine=xray live server switch needs restart: {}",
                error.message
            );
            return Ok(false);
        }
        session
            .live_outbound_tags
            .push((selected.into(), next_tag.clone(), outbound_hash));
    }
    if let Err(error) =
        run_live_routing_patch(&core, session.metrics_port + 2, &plan.forward_patch).await
    {
        log::warn!(
            "engine=xray live server routing needs restart: {}",
            error.message
        );
        return Ok(false);
    }
    session.pending_live_switch = Some(LiveSwitchRollback {
        previous_server_id: previous,
        previous_config_hash,
        previous_proxy_tag: previous_tag,
        previous_repository: previous_repo,
        routing_patch: plan.rollback_patch,
    });
    session.prepared_server_id = Some(selected.into());
    session.prepared_config_hash = Some(config_hash);
    session.prepared_repository = Some(stripped(repo));
    session.active_proxy_tag = Some(next_tag);
    log::info!(
        "engine=xray live server routing applied without restarting the core {} ms",
        switched_at.elapsed().as_millis()
    );
    Ok(true)
}

#[cfg(any(windows, target_os = "macos"))]
pub async fn rollback_live_server_switch(
    state: &Arc<AppState>,
    session: &mut XraySession,
) -> Result<bool> {
    let Some(rollback) = session.pending_live_switch.take() else {
        return Ok(false);
    };
    let core = state.core.read().await.clone();
    run_live_routing_patch(&core, session.metrics_port + 2, &rollback.routing_patch).await?;
    session.prepared_server_id = Some(rollback.previous_server_id);
    session.prepared_config_hash = Some(rollback.previous_config_hash);
    session.prepared_repository = Some(rollback.previous_repository);
    session.active_proxy_tag = Some(rollback.previous_proxy_tag);
    log::warn!("engine=xray live server switch rolled back after verification failed");
    Ok(true)
}

#[cfg(any(windows, target_os = "macos"))]
async fn run_live_outbound_add(
    core: &std::path::Path,
    api_port: u16,
    outbound: &serde_json::Value,
) -> Result<()> {
    let mut command = self::command(core);
    command.args([
        "api",
        "ado",
        &format!("--server=127.0.0.1:{api_port}"),
        "-timeout=5",
        "stdin:",
    ]);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|_| {
        BackendError::new("LIVE_SWITCH_FAILED", "Could not start Xray outbound update")
    })?;
    let bytes = serde_json::to_vec(&serde_json::json!({"outbounds":[outbound]}))
        .map_err(|_| BackendError::invalid())?;
    if let Some(mut input) = child.stdin.take() {
        input.write_all(&bytes).await.map_err(|_| {
            BackendError::new("LIVE_SWITCH_FAILED", "Could not send Xray outbound update")
        })?;
    }
    let output = tokio::time::timeout(Duration::from_secs(8), child.wait_with_output())
        .await
        .map_err(|_| BackendError::new("LIVE_SWITCH_FAILED", "Xray outbound update timed out"))?
        .map_err(|_| BackendError::new("LIVE_SWITCH_FAILED", "Xray outbound update failed"))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).to_ascii_lowercase();
        // A previous attempt may have added the tag before routing was rejected.
        if detail.contains("existing tag") {
            return Ok(());
        }
        log::warn!("engine=xray outbound command failed");
        return Err(BackendError::new(
            "LIVE_SWITCH_FAILED",
            "Xray rejected the outbound update",
        ));
    }
    Ok(())
}

#[cfg(any(windows, target_os = "macos"))]
pub(crate) async fn run_live_routing_patch(
    core: &std::path::Path,
    api_port: u16,
    patch: &serde_json::Value,
) -> Result<()> {
    let first = run_live_routing_patch_once(core, api_port, patch).await;
    if let Err(error) = &first {
        if error.message == "Xray rejected the routing update"
            || error.message == "Xray routing update timed out"
        {
            log::warn!("engine=xray routing update retry after {}", error.message);
            tokio::time::sleep(Duration::from_millis(200)).await;
            return run_live_routing_patch_once(core, api_port, patch).await;
        }
    }
    first
}

#[cfg(any(windows, target_os = "macos"))]
async fn run_live_routing_patch_once(
    core: &std::path::Path,
    api_port: u16,
    patch: &serde_json::Value,
) -> Result<()> {
    let mut command = self::command(core);
    command.args([
        "api",
        "adrules",
        &format!("--server=127.0.0.1:{api_port}"),
        "-timeout=5",
        "stdin:",
    ]);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|_| {
        BackendError::new("RULE_UPDATE_FAILED", "Could not start Xray routing update")
    })?;
    let bytes = serde_json::to_vec(&patch)
        .map_err(|_| BackendError::new("RULE_UPDATE_FAILED", "Could not encode routing update"))?;
    if let Some(mut input) = child.stdin.take() {
        input.write_all(&bytes).await.map_err(|_| {
            BackendError::new("RULE_UPDATE_FAILED", "Could not send routing update")
        })?;
    }
    let output = tokio::time::timeout(Duration::from_secs(8), child.wait_with_output())
        .await
        .map_err(|_| BackendError::new("RULE_UPDATE_FAILED", "Xray routing update timed out"))?
        .map_err(|_| BackendError::new("RULE_UPDATE_FAILED", "Xray routing update failed"))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).to_ascii_lowercase();
        let category = if detail.contains("geosite") || detail.contains("geoip") {
            "geo database rule"
        } else if detail.contains("deadline") || detail.contains("timeout") {
            "api timeout"
        } else if detail.contains("connect") || detail.contains("refused") {
            "api connection"
        } else if detail.contains("rule") || detail.contains("routing") {
            "invalid routing rule"
        } else {
            "unknown"
        };
        log::warn!("engine=xray routing command failed: {category}");
        return Err(BackendError::new(
            "RULE_UPDATE_FAILED",
            "Xray rejected the routing update",
        ));
    }
    Ok(())
}
#[cfg(windows)]
async fn start_tunnel(
    session: &mut XraySession,
    data: &std::path::Path,
    repository: &crate::storage::repository::Repository,
    selected: String,
    (proxy_port, metrics_port): (u16, u16),
    core_version: String,
    #[cfg(windows)] config_hash: [u8; 32],
) -> Result<()> {
    #[cfg(windows)]
    let outbound_hash = outbound_fingerprint(repository, &selected)?;
    #[cfg(windows)]
    let repository = stripped(repository);
    let tunnel_at = Instant::now();
    #[cfg(windows)]
    {
        if session
            .helper
            .as_ref()
            .is_some_and(|helper| helper.exited())
        {
            session.helper = None;
        }
        if session.helper.is_none() {
            session.helper = Some(
                crate::platform::helper::start_standby(
                    crate::platform::helper::StartTunnel {
                        core_version: core_version.clone(),
                        repository: (*repository).clone(),
                        selected: selected.clone(),
                        proxy_port,
                        metrics_port,
                    },
                    data,
                )
                .await?,
            );
            session.base_server_id = Some(selected.clone());
            session.base_outbound_hash = Some(outbound_hash);
            session.base_repository = Some(repository.clone());
            session.active_proxy_tag = Some("proxy".into());
            session.live_outbound_tags.clear();
            session.pending_live_switch = None;
        }
        session
            .helper
            .as_mut()
            .ok_or_else(BackendError::invalid)?
            .activate()
            .await?;
        session.prepared_server_id = Some(selected);
        session.prepared_config_hash = Some(config_hash);
        session.prepared_repository = Some(repository);
        session.prepared_core_version = Some(core_version);
        session.prepared_geo_hash = geo_fingerprint(data);
        session.proxy_port = proxy_port;
        session.metrics_port = metrics_port;
        session.started = Some(Instant::now());
        log::info!(
            "engine=xray VPN timing: TUN helper ready {} ms",
            tunnel_at.elapsed().as_millis()
        );
        let mut port_ready = false;
        for _ in 0..100 {
            if session.helper.as_ref().is_some_and(|h| h.exited()) {
                return Err(BackendError::new("TUN_FAILED", "Tunnel helper exited"));
            }
            if tokio::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, proxy_port))
                .await
                .is_ok()
            {
                port_ready = true;
                if crate::platform::network::verify_tunnel_routes().is_ok() {
                    log::info!(
                        "engine=xray VPN timing: TUN local port and routes ready {} ms",
                        tunnel_at.elapsed().as_millis()
                    );
                    return Ok(());
                }
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        if port_ready {
            crate::platform::network::verify_tunnel_routes()?;
        }
    }
    Err(BackendError::new("TUN_FAILED", "Tunnel was not ready"))
}
pub(crate) async fn start(
    core: &std::path::Path,
    session: &mut XraySession,
    bytes: &[u8],
    proxy_port: u16,
    metrics_port: u16,
) -> Result<()> {
    let core_at = Instant::now();
    #[cfg(any(windows, target_os = "macos"))]
    {
        session.child = Some(crate::platform::process::CoreProcess::spawn(core, bytes).await?);
        log::info!(
            "engine=xray VPN timing: Xray spawned {} ms",
            core_at.elapsed().as_millis()
        );
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    return Err(BackendError::new(
        "PLATFORM_UNSUPPORTED",
        "This build supports Windows networking only",
    ));
    session.proxy_port = proxy_port;
    session.metrics_port = metrics_port;
    session.started = Some(Instant::now());
    for _ in 0..100 {
        if session
            .child
            .as_mut()
            .ok_or_else(BackendError::invalid)?
            .try_wait()
            .map_err(|_| BackendError::new("CORE_CRASHED", "Could not inspect Xray process"))?
            .is_some()
        {
            return Err(BackendError::new(
                "CORE_START_FAILED",
                "Xray exited during startup; TUN requires administrator privileges",
            ));
        }
        if tokio::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, proxy_port))
            .await
            .is_ok()
        {
            log::info!(
                "engine=xray VPN timing: Xray local port ready {} ms",
                core_at.elapsed().as_millis()
            );
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    Err(BackendError::new(
        "CORE_START_FAILED",
        "Xray did not become ready",
    ))
}

/// Hands the configuration to LumenVpnService, which runs Xray on the
/// VpnService descriptor, then waits for the probe inbound like `start`.
#[cfg(target_os = "android")]
async fn start_android(
    core: &std::path::Path,
    repo: &Repository,
    selected: &str,
    session: &mut XraySession,
    proxy_port: u16,
    metrics_port: u16,
) -> Result<()> {
    let core_at = Instant::now();
    let config = crate::xray::config::build(repo, selected, proxy_port, metrics_port, true)?;
    let bytes = serde_json::to_vec(&config)
        .map_err(|_| BackendError::new("INVALID_CONFIG", "Could not serialize Xray config"))?;
    let assets = crate::xray::geo::assets_for_core(core);
    crate::platform::android::start(repo, selected, &bytes, &assets.to_string_lossy()).await?;
    log::info!(
        "engine=xray VPN timing: Android tunnel up {} ms",
        core_at.elapsed().as_millis()
    );
    session.proxy_port = proxy_port;
    session.metrics_port = metrics_port;
    session.started = Some(Instant::now());
    session.prepared_server_id = Some(selected.into());
    for _ in 0..200 {
        if tokio::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, proxy_port))
            .await
            .is_ok()
        {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    Err(BackendError::new(
        "CORE_START_FAILED",
        "Xray did not become ready",
    ))
}

pub async fn stop(session: &mut XraySession) -> Result<()> {
    let mut failures = Vec::new();
    // A process we failed to stop still owns the tunnel. Dropping the
    // bookkeeping would make the monitor treat that live tunnel as idle.
    let mut retained = false;
    // Only a tunnel this session started: Always-on or the tile may run one the
    // UI has not adopted yet.
    #[cfg(target_os = "android")]
    if session.started.is_some() || crate::platform::android::active() {
        if let Err(error) = crate::platform::android::stop().await {
            failures.push(error);
        }
    }
    #[cfg(windows)]
    if let Some(mut helper) = session.helper.take() {
        if !helper.exited() {
            if let Err(error) = helper.stop().await {
                // The helper refused to stop because routes are still installed
                // and it is still the process reading the adapter. Keep the pipe.
                if !helper.exited() {
                    session.helper = Some(helper);
                    retained = true;
                }
                failures.push(error);
            }
        }
    }
    if let Some(mut child) = session.child.take() {
        if !matches!(child.try_wait(), Ok(Some(_))) && child.kill().await.is_err() {
            failures.push(BackendError::new(
                "CORE_STOP_FAILED",
                "Cannot stop Xray process",
            ));
            session.child = Some(child);
            retained = true;
        }
    }
    if !retained {
        #[cfg(any(windows, target_os = "macos", target_os = "android"))]
        {
            session.prepared_server_id = None;
        }
        #[cfg(any(windows, target_os = "macos"))]
        {
            session.prepared_config_hash = None;
            session.prepared_core_version = None;
            session.prepared_geo_hash = None;
            session.base_server_id = None;
            session.base_outbound_hash = None;
            session.active_proxy_tag = None;
            session.live_outbound_tags.clear();
            session.pending_live_switch = None;
            session.prepared_repository = None;
            session.base_repository = None;
        }
        session.started = None;
    }
    for failure in &failures {
        log::warn!("engine=xray VPN cleanup incomplete: {}", failure.code);
    }
    failures.into_iter().next().map_or(Ok(()), Err)
}

/// Traffic counters prove that the native checks really carried data through the core.
#[cfg(any(test, all(windows, debug_assertions)))]
pub async fn counters(client: &reqwest::Client, port: u16) -> Result<(u64, u64)> {
    let error = || {
        BackendError::new(
            "METRICS_UNAVAILABLE",
            "Xray traffic metrics are unavailable",
        )
    };
    let response = client
        .get(format!("http://127.0.0.1:{port}/debug/vars"))
        .send()
        .await
        .map_err(|_| error())?;
    let data: serde_json::Value = response.json().await.map_err(|_| error())?;
    let outbound = data["stats"]["outbound"].as_object().ok_or_else(error)?;
    let mut up = 0u64;
    let mut down = 0u64;
    for (tag, stats) in outbound {
        if !["direct", "block", "metrics"].contains(&tag.as_str()) {
            up = up.saturating_add(stats["uplink"].as_u64().unwrap_or(0));
            down = down.saturating_add(stats["downlink"].as_u64().unwrap_or(0));
        }
    }
    Ok((up, down))
}

pub struct XrayBackend;
impl CoreBackend for XrayBackend {
    fn validate(&self, repo: &Repository, selected: &str) -> Result<()> {
        supported(repo)?;
        crate::xray::config::build(repo, selected, 10808, 10809, tun_mode(repo))?;
        Ok(())
    }
    fn validate_runtime<'a>(
        &'a self,
        state: &'a AppState,
        repo: &'a Repository,
        selected: &'a str,
    ) -> BackendFuture<'a, ()> {
        Box::pin(async move {
            supported(repo)?;
            let config = crate::xray::config::build(repo, selected, 10808, 10809, tun_mode(repo))?;
            let bytes = serde_json::to_vec(&config).map_err(|_| BackendError::invalid())?;
            #[cfg(target_os = "android")]
            {
                let assets = crate::xray::geo::assets_for_core(&state.core.read().await);
                crate::platform::android::validate(&bytes, &assets.to_string_lossy())
                    .await
                    .map_err(|output| rejection(&output))
            }
            #[cfg(not(target_os = "android"))]
            validate(&state.core.read().await, &bytes).await
        })
    }
    fn connect<'a>(
        &self,
        state: &'a Arc<AppState>,
        repo: &'a Repository,
        selected: &'a str,
        session: &'a mut BackendSession,
    ) -> BackendFuture<'a, ()> {
        Box::pin(async move {
            let s = session;
            supported(repo)?;
            let tun = tun_mode(repo);
            #[cfg(any(windows, target_os = "macos"))]
            let hash = configuration_fingerprint(repo, selected, tun)?;
            let core = state.core.read().await.clone();
            let version = core
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|s| s.to_str())
                .unwrap_or(crate::xray::bundle::VERSION);
            #[cfg(any(windows, target_os = "macos"))]
            let current_geo = geo_fingerprint(&state.data);
            // A running core (active or prepared TUN, connected proxy) takes server
            // and routing changes through its API. Its geo data is already loaded.
            #[cfg(any(windows, target_os = "macos"))]
            if s.is_tun == tun
                && api_available(s)
                && assets_match(s, version, current_geo)
                && update_running_core(state, repo, selected, hash, s).await
                && s.started.is_some()
            {
                return Ok(());
            }
            #[cfg(windows)]
            let reuse = tun
                && s.started.is_none()
                && s.prepared_server_id.as_deref() == Some(selected)
                && s.prepared_config_hash == Some(hash)
                && assets_match(s, version, current_geo)
                && s.helper
                    .as_ref()
                    .is_some_and(|h| !h.exited() && h.is_standby());
            #[cfg(not(windows))]
            let reuse = false;
            let uses_geo = repo.settings.dns_policy.enabled
                || (repo.routing_mode == "rules"
                    && repo.rules.iter().any(|r| {
                        r.enabled
                            && matches!(
                                r.matcher,
                                crate::storage::repository::RuleMatcher::Geosite
                                    | crate::storage::repository::RuleMatcher::Geoip
                            )
                    }));
            if uses_geo && !reuse {
                // Hashing both databases is disk-bound; keep it off the async workers.
                let data = state.data.clone();
                let valid =
                    tokio::task::spawn_blocking(move || crate::xray::geo::info(&data).valid)
                        .await
                        .unwrap_or(false);
                if !valid {
                    return Err(BackendError::new(
                        "GEO_DATA_CORRUPTED",
                        "Geo databases must pass validation before connecting",
                    ));
                }
            }
            let (proxy_port, metrics_port) = if reuse {
                (s.proxy_port, s.metrics_port)
            } else {
                ports()?
            };
            let bytes = if tun {
                Vec::new()
            } else {
                #[allow(unused_mut)]
                let mut config =
                    crate::xray::config::build(repo, selected, proxy_port, metrics_port, false)?;
                #[cfg(any(windows, target_os = "macos"))]
                crate::xray::config::enable_api(&mut config, metrics_port)?;
                serde_json::to_vec(&config).map_err(|_| {
                    BackendError::new("INVALID_CONFIG", "Could not serialize Xray config")
                })?
            };
            #[cfg(windows)]
            {
                let alive = tun
                    && s.helper
                        .as_ref()
                        .is_some_and(|helper| !helper.exited() && helper.is_standby());
                match helper_plan(alive, reuse) {
                    HelperPlan::Reuse => {}
                    HelperPlan::Reload => {
                        let repository = stripped(repo);
                        let request = crate::platform::helper::StartTunnel {
                            core_version: version.to_owned(),
                            repository: (*repository).clone(),
                            selected: selected.into(),
                            proxy_port,
                            metrics_port,
                        };
                        s.helper
                            .as_mut()
                            .ok_or_else(BackendError::invalid)?
                            .reload(request, &state.data)
                            .await?;
                        note_prepared(
                            s,
                            selected,
                            hash,
                            version,
                            current_geo,
                            outbound_fingerprint(repo, selected)?,
                            repository,
                            proxy_port,
                            metrics_port,
                        );
                    }
                    HelperPlan::Launch => {
                        stop(s).await?;
                    }
                }
            }
            // Android replaces the running core and keeps the VPN interface up,
            // so a server switch never lets traffic out around the tunnel.
            #[cfg(not(any(windows, target_os = "android")))]
            if !reuse {
                stop(s).await?;
            }
            s.is_tun = tun;
            if tun {
                #[cfg(windows)]
                return start_tunnel(
                    s,
                    &state.data,
                    repo,
                    selected.into(),
                    (proxy_port, metrics_port),
                    version.into(),
                    hash,
                )
                .await;
                #[cfg(target_os = "android")]
                return start_android(&core, repo, selected, s, proxy_port, metrics_port).await;
                #[cfg(not(any(windows, target_os = "android")))]
                return Err(BackendError::new("TUN_FAILED", "Tunnel was not ready"));
            }
            start(&core, s, &bytes, proxy_port, metrics_port).await?;
            #[cfg(any(windows, target_os = "macos"))]
            {
                let repository = stripped(repo);
                s.base_server_id = Some(selected.into());
                s.base_outbound_hash = Some(outbound_fingerprint(repo, selected)?);
                s.base_repository = Some(repository.clone());
                s.prepared_repository = Some(repository);
                s.prepared_server_id = Some(selected.into());
                s.prepared_config_hash = Some(hash);
                s.prepared_core_version = Some(version.into());
                s.prepared_geo_hash = current_geo;
                s.active_proxy_tag = Some("proxy".into());
                s.live_outbound_tags.clear();
                s.pending_live_switch = None;
            }
            Ok(())
        })
    }
    fn prewarm<'a>(
        &self,
        state: &'a Arc<AppState>,
        repo: &'a Repository,
        selected: Option<&'a str>,
        session: &'a mut BackendSession,
    ) -> BackendFuture<'a, ()> {
        Box::pin(async move {
            let s = session;
            if s.started.is_some() {
                return Ok(());
            }
            if !tun_mode(repo) || selected.is_none() {
                return stop(s).await;
            }
            supported(repo)?;
            #[cfg(not(windows))]
            let _ = state;
            #[cfg(windows)]
            {
                let selected = selected.ok_or_else(BackendError::invalid)?;
                let hash = configuration_fingerprint(repo, selected, true)?;
                let outbound_hash = outbound_fingerprint(repo, selected)?;
                let geo = geo_fingerprint(&state.data);
                let core = state.core.read().await.clone();
                let version = core
                    .parent()
                    .and_then(|p| p.file_name())
                    .and_then(|s| s.to_str())
                    .unwrap_or(crate::xray::bundle::VERSION)
                    .to_owned();
                let alive = s
                    .helper
                    .as_ref()
                    .is_some_and(|h| !h.exited() && h.is_standby());
                // Rules, routing mode and server selection change through the
                // prepared core's API: no new elevated helper while disconnected.
                let config_matches = if alive && s.is_tun && assets_match(s, &version, geo) {
                    update_running_core(state, repo, selected, hash, s).await
                } else {
                    false
                };
                match helper_plan(alive, config_matches) {
                    HelperPlan::Reuse => {
                        s.pending_live_switch = None;
                        return Ok(());
                    }
                    HelperPlan::Reload => {
                        let (proxy_port, metrics_port) = ports()?;
                        let repository = stripped(repo);
                        let request = crate::platform::helper::StartTunnel {
                            core_version: version.clone(),
                            repository: (*repository).clone(),
                            selected: selected.into(),
                            proxy_port,
                            metrics_port,
                        };
                        s.helper
                            .as_mut()
                            .ok_or_else(BackendError::invalid)?
                            .reload(request, &state.data)
                            .await?;
                        note_prepared(
                            s,
                            selected,
                            hash,
                            &version,
                            geo,
                            outbound_hash,
                            repository,
                            proxy_port,
                            metrics_port,
                        );
                        log::info!("engine=xray TUN reloaded without a new elevation");
                        return Ok(());
                    }
                    HelperPlan::Launch => {
                        if s.helper.is_some() {
                            stop(s).await?;
                        }
                        let (proxy_port, metrics_port) = ports()?;
                        let repository = stripped(repo);
                        let request = crate::platform::helper::StartTunnel {
                            core_version: version.clone(),
                            repository: (*repository).clone(),
                            selected: selected.into(),
                            proxy_port,
                            metrics_port,
                        };
                        s.helper = Some(
                            crate::platform::helper::start_standby(request, &state.data).await?,
                        );
                        note_prepared(
                            s,
                            selected,
                            hash,
                            &version,
                            geo,
                            outbound_hash,
                            repository,
                            proxy_port,
                            metrics_port,
                        );
                        log::info!("engine=xray TUN prepared without installing routes");
                    }
                }
            }
            Ok(())
        })
    }
    fn disconnect<'a>(
        &self,
        session: &'a mut BackendSession,
        keep_warm: bool,
    ) -> BackendFuture<'a, ()> {
        Box::pin(async move {
            let s = session;
            #[cfg(not(windows))]
            let _ = keep_warm;
            #[cfg(windows)]
            if keep_warm {
                if let Some(helper) = s.helper.as_mut().filter(|h| h.is_standby() && !h.exited()) {
                    if s.started.is_some() || !crate::platform::network::tunnel_defaults_absent() {
                        if let Err(error) = helper.deactivate().await {
                            // Routes are still installed, so the helper keeps reading
                            // the adapter. Killing it here would black-hole the system.
                            log::warn!("engine=xray TUN standby release failed: {}", error.code);
                            return Err(error);
                        }
                    }
                    s.started = None;
                    return Ok(());
                }
            }
            stop(s).await
        })
    }
    fn update_routing<'a>(
        &self,
        state: &'a Arc<AppState>,
        selected: &'a str,
        before: &'a Repository,
        after: &'a Repository,
        session: &'a mut BackendSession,
    ) -> BackendFuture<'a, crate::engine::LiveRoutingStatus> {
        Box::pin(async move {
            #[cfg(any(windows, target_os = "macos"))]
            {
                apply_live_rules(state, selected, Some(before), after, session).await
            }
            #[cfg(not(any(windows, target_os = "macos")))]
            {
                Ok(crate::engine::LiveRoutingStatus::NeedsConnect)
            }
        })
    }
}

/// Reinstall the default routes of a connected TUN session in its elevated
/// helper, after Windows stopped selecting LumenTun. No new elevation.
#[cfg(windows)]
pub async fn reassert_routes(session: &mut XraySession) -> Result<()> {
    if session.started.is_none() || !session.is_tun {
        return Err(BackendError::new("TUN_FAILED", "Tunnel is not connected"));
    }
    let helper = session
        .helper
        .as_mut()
        .filter(|helper| helper.is_standby() && !helper.exited())
        .ok_or_else(|| BackendError::new("TUN_FAILED", "Tunnel helper is unavailable"))?;
    helper.deactivate().await?;
    helper.activate().await
}

pub fn failed(session: &mut XraySession) -> bool {
    #[cfg(target_os = "android")]
    if session.started.is_some() {
        return crate::platform::android::failed();
    }
    #[cfg(windows)]
    if session.helper.as_ref().is_some_and(|h| h.exited()) {
        return true;
    }
    session
        .child
        .as_mut()
        .is_some_and(|c| !matches!(c.try_wait(), Ok(None)))
}
