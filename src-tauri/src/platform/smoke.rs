//! Opt-in developer smoke. Never compiled into release builds.
use crate::{
    domain::errors::{BackendError, Result},
    storage::repository::{Repository, Rule},
};
use std::time::Duration;
async fn public_ip() -> Result<String> {
    resolve_ip(None).await
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
        let mut helper = super::helper::start(super::helper::StartTunnel {
            core_version: crate::xray::core::VERSION.into(),
            repository: repo.clone(),
            selected: id.clone(),
            proxy_port,
            metrics_port,
        }, data.path())
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
            let (up, down) =
                crate::services::stats_service::counters(&client, metrics_port).await?;
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
