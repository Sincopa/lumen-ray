//! Opt-in developer smoke. Never compiled into release builds.
use crate::{
    domain::errors::{BackendError, Result},
    storage::repository::{Repository, Rule},
};
use std::time::Duration;
async fn public_ip() -> Result<String> {
    resolve_ip(None).await
}
async fn steam_reachable() -> Result<()> {
    tokio::time::timeout(
        Duration::from_secs(8),
        tokio::net::TcpStream::connect(("api.steampowered.com", 443)),
    )
    .await
    .map_err(|_| error())?
    .map_err(|_| error())?;
    Ok(())
}
async fn routing_rule_count(core: &std::path::Path, api_port: u16) -> Result<usize> {
    let output = tokio::time::timeout(
        Duration::from_secs(5),
        crate::engine::xray::command(core)
            .args(["api", "lsrules", &format!("--server=127.0.0.1:{api_port}")])
            .output(),
    )
    .await
    .map_err(|_| error())?
    .map_err(|_| error())?;
    if !output.status.success() {
        return Err(BackendError::new(
            "SMOKE_FAILED",
            "Routing API list command failed",
        ));
    }
    let response: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|_| BackendError::new("SMOKE_FAILED", "Routing API list response is not JSON"))?;
    response["rules"].as_array().map(Vec::len).ok_or_else(|| {
        BackendError::new(
            "SMOKE_FAILED",
            "Routing API list response has no rules array",
        )
    })
}
async fn resolve_ip(proxy: Option<u16>) -> Result<String> {
    let mut builder = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(15));
    if let Some(port) = proxy {
        builder = builder.proxy(
            reqwest::Proxy::all(format!("socks5h://127.0.0.1:{port}")).map_err(|_| error())?,
        );
    }
    let client = builder.build().map_err(|_| error())?;
    let text = client
        .get("https://www.cloudflare.com/cdn-cgi/trace")
        .send()
        .await
        .map_err(|_| error())?
        .text()
        .await
        .map_err(|_| error())?;
    let ip = text
        .lines()
        .find_map(|l| l.strip_prefix("ip="))
        .ok_or_else(error)?;
    ip.trim().parse::<std::net::IpAddr>().map_err(|_| error())?;
    Ok(ip.trim().into())
}
fn error() -> BackendError {
    BackendError::new("SMOKE_FAILED", "Native traffic verification failed")
}
fn port() -> Result<u16> {
    std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .map(|a| a.port())
        .map_err(|_| error())
}
pub async fn run() -> Result<()> {
    let source = std::fs::read_to_string(
        std::env::var_os("LUMEN_TEST_SUBSCRIPTION_FILE").ok_or_else(error)?,
    )
    .map_err(|_| error())?;
    let client = crate::subscription::fetcher::SubscriptionClient::ephemeral()?;
    let body = client.fetch_with_metadata(source.trim()).await?.body;
    let parsed = crate::subscription::parser::parse(&body, "native-smoke")?;
    let node = parsed.nodes.first().ok_or_else(error)?;
    let id = node.id.clone();
    let mut repo = Repository {
        nodes: parsed.nodes,
        ..Default::default()
    };
    let data = tempfile::tempdir().map_err(|_| error())?;
    let resources = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/core");
    crate::xray::geo::ensure_initialized(&resources, data.path())?;
    let original = public_ip().await?;
    for rules in [false, true] {
        repo.routing_mode = if rules { "rules" } else { "global" }.into();
        repo.rules = if rules {
            vec![Rule {
                id: "smoke-curl".into(),
                name: "curl.exe".into(),
                matcher: crate::storage::repository::RuleMatcher::Application,
                server_id: id.clone(),
                enabled: true,
                preset_id: None,
                is_domain: false,
            }]
        } else {
            vec![]
        };
        let proxy_port = port()?;
        let metrics_port = port()?;
        let mut helper = super::helper::start(
            super::helper::StartTunnel {
                core_version: crate::xray::bundle::VERSION.into(),
                repository: repo.clone(),
                selected: id.clone(),
                proxy_port,
                metrics_port,
            },
            data.path(),
        )
        .await?;
        let result = async {
            for _ in 0..100 {
                if helper.exited() {
                    return Err(error());
                }
                if tokio::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, proxy_port))
                    .await
                    .is_ok()
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            let vpn = resolve_ip(Some(proxy_port)).await?;
            if vpn == original {
                return Err(BackendError::new(
                    "SMOKE_FAILED",
                    "Test server egress equals original IP; cannot prove routing",
                ));
            }
            let system = public_ip().await?;
            println!(
                "Route diagnostics: ready={}, system_matches_original={}, same_address_family={}",
                super::network::tunnel_ready(),
                system == original,
                system.contains(':') == vpn.contains(':')
            );
            if rules {
                if system != original {
                    return Err(BackendError::new(
                        "SMOKE_FAILED",
                        "Unselected application did not use DIRECT",
                    ));
                }
                let curl = std::env::var_os("SystemRoot")
                    .map(std::path::PathBuf::from)
                    .ok_or_else(error)?
                    .join("System32/curl.exe");
                let mut command = tokio::process::Command::new(curl);
                command.creation_flags(0x08000000);
                let output = command
                    .args([
                        "--silent",
                        "--fail",
                        "--max-time",
                        "15",
                        "--noproxy",
                        "*",
                        "https://www.cloudflare.com/cdn-cgi/trace",
                    ])
                    .output()
                    .await
                    .map_err(|_| error())?;
                let text = String::from_utf8_lossy(&output.stdout);
                if !output.status.success()
                    || text.lines().find_map(|l| l.strip_prefix("ip=")) != Some(vpn.as_str())
                {
                    return Err(BackendError::new(
                        "SMOKE_FAILED",
                        "Selected application did not use VPN",
                    ));
                }
            } else if system != vpn {
                return Err(BackendError::new(
                    "SMOKE_FAILED",
                    "Global system traffic did not use VPN",
                ));
            }
            let client = reqwest::Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(2))
                .build()
                .map_err(|_| error())?;
            let (up, down) = crate::engine::xray::counters(&client, metrics_port).await?;
            if up == 0 || down == 0 {
                return Err(error());
            }
            Ok(())
        }
        .await;
        let cleanup = helper.stop().await;
        result?;
        cleanup?;
        if public_ip().await? != original {
            return Err(BackendError::new(
                "SMOKE_FAILED",
                "Network not restored after disconnect",
            ));
        }
        println!("{}: verified system/application IP path, traffic counters, disconnect and original IP restoration",if rules{"APPLICATION"}else{"GLOBAL"});
    }
    Ok(())
}

/// Offline route-cycle check for the prepared adapter; no subscription or
/// remote server is contacted. Requires Windows elevation through the helper.
pub async fn warm_route_cycle() -> Result<()> {
    let original = public_ip().await?;
    let node = crate::subscription::uri::parse(
        "vless://00000000-0000-4000-8000-000000000001@example.com:443?encryption=none&security=tls&type=tcp&sni=example.com#offline-smoke",
        "offline-smoke",
    )?;
    let selected = node.id.clone();
    let repository = Repository {
        nodes: vec![node],
        ..Default::default()
    };
    let data = tempfile::tempdir().map_err(|_| error())?;
    let runtime = crate::runtime::resolve()?;
    crate::xray::geo::ensure_initialized(&runtime, data.path())?;
    let (proxy_port, metrics_port) = crate::engine::xray::ports()?;
    let started = std::time::Instant::now();
    let request = super::helper::StartTunnel {
        core_version: crate::xray::bundle::VERSION.into(),
        repository,
        selected,
        proxy_port,
        metrics_port,
    };
    let mut helper = super::helper::start_standby(request, data.path()).await?;
    let prepared_ms = started.elapsed().as_millis();
    let cycle = async {
        if public_ip().await? != original {
            return Err(BackendError::new(
                "SMOKE_FAILED",
                "Direct Internet changed while tunnel was prepared",
            ));
        }
        let mut activation_ms = Vec::new();
        for _ in 0..3 {
            if super::network::tunnel_ready() {
                return Err(error());
            }
            let activated = std::time::Instant::now();
            helper.activate().await?;
            activation_ms.push(activated.elapsed().as_millis());
            super::network::verify_tunnel_routes()?;
            helper.deactivate().await?;
            if super::network::tunnel_ready() {
                return Err(error());
            }
            if public_ip().await? != original {
                return Err(BackendError::new(
                    "SMOKE_FAILED",
                    "Direct Internet changed after tunnel disconnect",
                ));
            }
        }
        println!("WARM_TUN_ROUTE_OK prepare={prepared_ms}ms activate={activation_ms:?}ms");
        Ok(())
    }
    .await;
    let cleanup = helper.stop().await;
    cycle?;
    cleanup
}

/// Check the background adapter, ordinary Internet and Xray's live routing API
/// without installing a VPN default route or contacting a private server.
pub async fn warm_standby_isolation() -> Result<()> {
    eprintln!("STANDBY_CHECK baseline Internet");
    let original = public_ip().await?;
    eprintln!("STANDBY_CHECK baseline Steam endpoint");
    steam_reachable().await?;
    let node = crate::subscription::uri::parse(
        "vless://00000000-0000-4000-8000-000000000001@example.com:443?encryption=none&security=tls&type=tcp&sni=example.com#offline-smoke",
        "offline-smoke",
    )?;
    let selected = node.id.clone();
    let mut before = Repository {
        nodes: vec![node],
        ..Default::default()
    };
    before.routing_mode = "rules".into();
    before.rules.push(Rule {
        id: "steam-smoke".into(),
        name: "steam.exe".into(),
        matcher: crate::storage::repository::RuleMatcher::Application,
        server_id: "subscription".into(),
        enabled: true,
        preset_id: None,
        is_domain: false,
    });
    let mut after = before.clone();
    after.rules[0].enabled = false;
    let mut global = after.clone();
    global.routing_mode = "global".into();
    let data = tempfile::tempdir().map_err(|_| error())?;
    let runtime = crate::runtime::resolve()?;
    let core = crate::xray::bundle::prepare(&runtime, data.path())?;
    let (proxy_port, metrics_port) = crate::engine::xray::ports()?;
    let request = super::helper::StartTunnel {
        core_version: crate::xray::bundle::VERSION.into(),
        repository: before.clone(),
        selected: selected.clone(),
        proxy_port,
        metrics_port,
    };
    let mut helper = super::helper::start_standby(request, data.path()).await?;
    eprintln!("STANDBY_CHECK prepared adapter isolation");
    let result = async {
        if !super::network::standby_isolated() {
            return Err(error());
        }
        if public_ip().await? != original {
            return Err(error());
        }
        steam_reachable().await?;
        eprintln!("STANDBY_CHECK live rule update");
        let old_count = routing_rule_count(&core, metrics_port + 2).await?;
        let patch = crate::xray::config::live_routing_patch(&before, &after, &selected, true)?
            .ok_or_else(error)?;
        crate::engine::xray::run_live_routing_patch(&core, metrics_port + 2, &patch).await?;
        let updated_count = routing_rule_count(&core, metrics_port + 2).await?;
        if updated_count + 1 != old_count {
            return Err(BackendError::new(
                "SMOKE_FAILED",
                format!("Rule count after update: {updated_count}, before: {old_count}"),
            ));
        }
        eprintln!("STANDBY_CHECK rules to global");
        let mode_patch = crate::xray::config::live_routing_patch(&after, &global, &selected, true)?
            .ok_or_else(error)?;
        crate::engine::xray::run_live_routing_patch(&core, metrics_port + 2, &mode_patch).await?;
        let global_count = routing_rule_count(&core, metrics_port + 2).await?;
        if global_count >= old_count {
            return Err(error());
        }
        eprintln!("STANDBY_CHECK global to rules");
        let back_patch = crate::xray::config::live_routing_patch(&global, &after, &selected, true)?
            .ok_or_else(error)?;
        crate::engine::xray::run_live_routing_patch(&core, metrics_port + 2, &back_patch).await?;
        if routing_rule_count(&core, metrics_port + 2).await? + 1 != old_count {
            return Err(error());
        }
        if public_ip().await? != original || !super::network::standby_isolated() {
            return Err(error());
        }
        steam_reachable().await?;
        Ok(())
    }
    .await;
    eprintln!("STANDBY_CHECK cleanup");
    let cleanup = helper.stop().await;
    result?;
    cleanup?;
    if public_ip().await? != original {
        return Err(error());
    }
    steam_reachable().await?;
    println!(
        "WARM_TUN_STANDBY_OK direct Internet, Steam endpoint, live rules, mode changes and cleanup"
    );
    Ok(())
}

/// Opt-in live check against the locally selected encrypted repository node.
/// Prints no server address, credentials, subscription URL, or public IP.
pub async fn warm_live() -> Result<()> {
    let data = std::path::PathBuf::from(std::env::var_os("LUMEN_TEST_DATA_DIR").ok_or_else(error)?);
    let mut repository =
        crate::storage::repository::RepositoryStore::new(&data.join("data"))?.load()?;
    let selected = repository.selected_server_id.clone().ok_or_else(error)?;
    repository.sources.clear();
    let (proxy_port, metrics_port) = crate::engine::xray::ports()?;
    let request = super::helper::StartTunnel {
        core_version: crate::xray::updater::current(&data.join("core")),
        repository,
        selected,
        proxy_port,
        metrics_port,
    };
    let mut helper = super::helper::start_standby(request, &data).await?;
    let result = async {
        if super::network::tunnel_ready() {
            return Err(error());
        }
        helper.activate().await?;
        super::network::verify_tunnel_routes()?;
        let ip = crate::services::vpn_service::public_ip(proxy_port).await?;
        if ip.is_empty() {
            return Err(error());
        }
        helper.deactivate().await?;
        if super::network::tunnel_ready() {
            return Err(error());
        }
        println!("WARM_TUN_LIVE_OK");
        Ok(())
    }
    .await;
    let cleanup = helper.stop().await;
    result?;
    cleanup
}
