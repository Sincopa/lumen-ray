use super::*;

#[test]
#[ignore = "Requires an explicit local DPAPI repository directory and a browser; never modifies user data"]
fn private_geo_video_check() {
    let source = std::env::var_os("LUMEN_GEO_TEST_REPOSITORY").expect("Set repository directory");
    let saved = RepositoryStore::new(std::path::Path::new(&source))
        .unwrap()
        .load()
        .unwrap();
    let selected = saved
        .selected_server_id
        .clone()
        .expect("Select a server first");
    let resources = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/core");
    let isolated = tempfile::tempdir().unwrap();
    let core = bundle::prepare(&resources, isolated.path()).unwrap();
    for mode in ["baseline-rules", "global", "fixed-rules"] {
        let mut repo = saved.clone();
        repo.routing_mode = if mode == "global" { "global" } else { "rules" }.into();
        if mode == "fixed-rules" {
            let presets: std::collections::BTreeSet<_> = repo
                .rules
                .iter()
                .filter_map(|r| r.preset_id.clone())
                .collect();
            repo.rules.retain(|r| r.preset_id.is_none());
            for preset in presets {
                repo.rules.extend(
                    crate::services::catalog_service::generate_preset_rules(
                        &preset,
                        "subscription",
                    )
                    .unwrap(),
                );
            }
        }
        let mut value =
            serde_json::to_value(config::build(&repo, &selected, 19876, 19877, false).unwrap())
                .unwrap();
        if mode == "baseline-rules" {
            value.as_object_mut().unwrap().remove("dns");
            value["outbounds"]
                .as_array_mut()
                .unwrap()
                .retain(|o| o["tag"] != "dns");
            let rules = value["routing"]["rules"].as_array_mut().unwrap();
            rules.retain(|r| {
                r["outboundTag"] != "dns" && r["inboundTag"] != serde_json::json!(["dns-query"])
            });
            rules.insert(
                rules.len() - 1,
                serde_json::json!({"type":"field","port":"53","outboundTag":"proxy"}),
            );
            value["routing"]["domainStrategy"] = serde_json::json!("IPIfNonMatch");
        }
        value["log"]["loglevel"] = serde_json::json!("info");
        let path = isolated.path().join("private-config.json");
        std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        let status = std::process::Command::new("node")
            .arg(resources.join("../../../scripts/geo-video-check.mjs"))
            .args([
                core.as_os_str(),
                crate::xray::geo::assets_for_core(&core).as_os_str(),
                path.as_os_str(),
                std::ffi::OsStr::new(mode),
            ])
            .status()
            .unwrap();
        assert!(status.success(), "Browser diagnostic runner failed");
    }
}

#[tokio::test]
#[ignore = "Requires local private subscription and Internet access"]
async fn private_subscription_config_validation() {
    let filename =
        std::env::var_os("LUMEN_TEST_SUBSCRIPTION_FILE").expect("Set local test subscription file");
    let source_file = std::fs::read_to_string(filename).unwrap();
    let lines: Vec<_> = source_file
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    let sources = if lines.len() > 1
        && lines
            .iter()
            .all(|line| line.starts_with("https://") || line.starts_with("http://"))
    {
        lines
    } else {
        vec![source_file.trim()]
    };
    let mut nodes = Vec::new();
    let mut diagnostic_count = 0;
    let client = crate::subscription::fetcher::SubscriptionClient::ephemeral().unwrap();
    for (index, source) in sources.into_iter().enumerate() {
        let body = client.fetch(source).await.unwrap();
        let parsed = parser::parse(&body, &format!("test-{index}")).unwrap();
        diagnostic_count += parsed.diagnostics.len();
        nodes.extend(parsed.nodes);
    }
    eprintln!(
        "Supported nodes: {}; unsupported nodes: {}",
        nodes.len(),
        diagnostic_count
    );
    let directory = tempfile::tempdir().unwrap();
    let core = bundle::prepare(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/core"),
        directory.path(),
    )
    .unwrap();
    let repo = Repository {
        nodes,
        ..Default::default()
    };
    let mut failures = std::collections::BTreeMap::new();
    for node in &repo.nodes {
        let bytes =
            serde_json::to_vec(&config::build(&repo, &node.id, 10808, 10809, false).unwrap())
                .unwrap();
        if let Err(error) = crate::engine::xray::validate(&core, &bytes).await {
            use tokio::io::AsyncWriteExt;
            let mut child = crate::engine::xray::command(&core)
                .args(["run", "-test", "-config", "stdin:"])
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap();
            child.stdin.take().unwrap().write_all(&bytes).await.unwrap();
            let output = child.wait_with_output().await.unwrap();
            let output = format!(
                "{} {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            let mut failure = output
                .lines()
                .find(|s| s.contains("Failed to start"))
                .unwrap_or("unknown")
                .to_owned();
            fn strings(v: &serde_json::Value, out: &mut Vec<String>) {
                match v {
                    serde_json::Value::String(s) => {
                        if !s.is_empty() {
                            out.push(s.clone());
                        }
                    }
                    serde_json::Value::Object(m) => m.values().for_each(|v| strings(v, out)),
                    serde_json::Value::Array(a) => a.iter().for_each(|v| strings(v, out)),
                    _ => {}
                }
            }
            let mut values = vec![];
            strings(&serde_json::to_value(node).unwrap(), &mut values);
            values.sort_by_key(|s| std::cmp::Reverse(s.len()));
            for value in values {
                failure = failure.replace(&value, "[redacted]");
            }
            eprintln!("Redacted validator failure: {failure}");
            *failures.entry(error.message).or_insert(0) += 1;
        }
    }
    assert!(
        failures.is_empty(),
        "Config failure categories: {failures:?}"
    );
}

#[tokio::test]
#[ignore = "Requires local private subscription and Internet access; starts isolated SOCKS probes"]
async fn private_proxy_traffic_smoke() {
    let source = std::fs::read_to_string(
        std::env::var_os("LUMEN_TEST_SUBSCRIPTION_FILE").expect("Set test file"),
    )
    .unwrap();
    let client = crate::subscription::fetcher::SubscriptionClient::ephemeral().unwrap();
    let body = client.fetch(source.trim()).await.unwrap();
    let parsed = parser::parse(&body, "smoke").unwrap();
    let directory = tempfile::tempdir().unwrap();
    let core = bundle::prepare(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/core"),
        directory.path(),
    )
    .unwrap();
    let repo = Repository {
        nodes: parsed.nodes,
        ..Default::default()
    };
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .unwrap();
    let mut connected = false;
    for (index, node) in repo.nodes.iter().take(5).enumerate() {
        let port = || {
            std::net::TcpListener::bind("127.0.0.1:0")
                .unwrap()
                .local_addr()
                .unwrap()
                .port()
        };
        let proxy = port();
        let metrics = port();
        let bytes =
            serde_json::to_vec(&config::build(&repo, &node.id, proxy, metrics, false).unwrap())
                .unwrap();
        crate::engine::xray::validate(&core, &bytes).await.unwrap();
        let mut session = crate::engine::xray::XraySession::default();
        let result = crate::engine::xray::start(&core, &mut session, &bytes, proxy, metrics).await;
        if result.is_ok() {
            let second = vpn_service::public_ip(proxy).await;
            let counters = crate::engine::xray::counters(&client, metrics).await;
            crate::engine::xray::stop(&mut session).await.unwrap();
            assert!(second.is_ok());
            let (up, down) = counters.unwrap();
            assert!(up > 0 && down > 0);
            assert!(
                tokio::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, proxy))
                    .await
                    .is_err()
            );
            eprintln!("Node {index}: HTTPS through VPN verified; real upload/download counters positive; listener closed after disconnect");
            connected = true;
            break;
        }
        crate::engine::xray::stop(&mut session).await.unwrap();
        eprintln!("Node {index}: unreachable (credentials redacted)");
    }
    assert!(
        connected,
        "No tested node could establish a verified connection"
    );
}

#[cfg(windows)]
#[test]
#[ignore = "Performance diagnostic; stages multiple isolated Xray runtimes"]
fn tunnel_asset_staging_benchmark() {
    let resources = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/core");
    let source = tempfile::tempdir().unwrap();
    bundle::prepare(&resources, source.path()).unwrap();
    let (metadata, geoip, geosite) = crate::xray::geo::snapshot(source.path()).unwrap();
    for _ in 0..3 {
        let target = tempfile::tempdir().unwrap();
        let started = std::time::Instant::now();
        crate::xray::geo::ensure_initialized(&resources, target.path()).unwrap();
        let geo_initialized = started.elapsed().as_millis();
        bundle::prepare(&resources, target.path()).unwrap();
        let prepared = started.elapsed().as_millis();
        crate::xray::geo::install_trusted_snapshot(
            target.path(),
            metadata.clone(),
            &geoip,
            &geosite,
        )
        .unwrap();
        eprintln!(
            "TUN staging: geo_init={} ms, core_files={} ms, transferred_geo={} ms",
            geo_initialized,
            prepared - geo_initialized,
            started.elapsed().as_millis() - prepared
        );

        let fast = tempfile::tempdir().unwrap();
        let fast_started = std::time::Instant::now();
        let executable = bundle::prepare_for_tunnel(&resources, fast.path()).unwrap();
        let fast_core = fast_started.elapsed().as_millis();
        assert!(executable.is_file());
        assert!(executable.parent().unwrap().join("wintun.dll").is_file());
        assert!(!crate::xray::geo::metadata_path(fast.path()).exists());
        crate::xray::geo::install_trusted_snapshot(fast.path(), metadata.clone(), &geoip, &geosite)
            .unwrap();
        assert!(crate::xray::geo::info(fast.path()).valid);
        eprintln!(
            "TUN fast staging: core={} ms, transferred_geo_and_verify={} ms",
            fast_core,
            fast_started.elapsed().as_millis() - fast_core
        );
    }
}
