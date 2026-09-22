use crate::{
    services::vpn_service,
    storage::{
        repository::{Repository, RepositoryStore},
        secure::{OsSecretStore, SecretStore},
    },
    subscription::{parser, uri},
    xray::{config, core},
};
const VLESS:&str="vless://00000000-0000-4000-8000-000000000001@example.com:443?type=ws&security=tls&sni=example.com&path=%2Fvpn#Example";
#[test]
fn uri_variants_and_stable_identity() {
    let node = uri::parse(VLESS, "sub").unwrap();
    assert_eq!(node.transport.path, "/vpn");
    assert_eq!(
        node.id,
        uri::parse(&VLESS.replace("#Example", "#Renamed"), "sub")
            .unwrap()
            .id
    );
    assert_ne!(node.id, uri::parse(VLESS, "other").unwrap().id);
    for (query, network) in [
        ("type=xhttp&security=tls", "xhttp"),
        ("type=grpc&security=tls&serviceName=test", "grpc"),
        (
            "type=raw&security=reality&sni=example.com&pbk=test&sid=abcd&flow=xtls-rprx-vision",
            "raw",
        ),
    ] {
        let input = format!("vless://00000000-0000-4000-8000-000000000001@example.com:443?{query}");
        assert_eq!(
            uri::parse(&input, "sub").unwrap().transport.network,
            network
        );
    }
    assert!(uri::parse(&format!("{VLESS}&allowInsecure=1"), "sub").is_ok()); // part of the fragment, not a query
    assert!(uri::parse("trojan://password@example.com:443?allowInsecure=1", "sub").is_err());
    assert!(uri::parse("vless://id@example.com:0", "sub").is_err());
}
#[test]
fn protocols_and_base64() {
    use base64::Engine;
    let vmess=base64::engine::general_purpose::STANDARD.encode(r#"{"v":"2","ps":"test","add":"example.com","port":443,"id":"00000000-0000-4000-8000-000000000001","net":"ws","tls":"tls"}"#);
    assert!(uri::parse(&format!("vmess://{vmess}"), "sub").is_ok());
    assert_eq!(
        uri::parse("trojan://a%40b@example.com:443", "sub")
            .unwrap()
            .credential,
        "a@b"
    );
    let auth = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode("aes-256-gcm:password");
    assert_eq!(
        uri::parse(&format!("ss://{auth}@example.com:8388"), "sub")
            .unwrap()
            .method,
        "aes-256-gcm"
    );
    let list = format!("{VLESS}\n{VLESS}\nhysteria2://not-supported");
    let parsed = parser::parse(
        &base64::engine::general_purpose::STANDARD.encode(list),
        "sub",
    )
    .unwrap();
    assert_eq!(parsed.nodes.len(), 1);
    assert_eq!(parsed.diagnostics.len(), 1);
}
#[test]
fn yaml_json_and_config_routing() {
    let yaml="proxies:\n  - name: test\n    type: trojan\n    server: example.com\n    port: 443\n    password: secret\n";
    let node = parser::parse(yaml, "sub").unwrap().nodes.remove(0);
    let outbound = config::outbound(&node, "test").unwrap();
    assert_eq!(
        parser::parse(&outbound.to_string(), "sub").unwrap().nodes[0].address,
        "example.com"
    );
    let repo = Repository {
        nodes: vec![node.clone()],
        routing_mode: "rules".into(),
        ..Default::default()
    };
    let generated =
        serde_json::to_value(config::build(&repo, &node.id, 1080, 1081, true).unwrap()).unwrap();
    assert_eq!(
        generated["routing"]["rules"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()["outboundTag"],
        "direct"
    );
    assert_eq!(
        generated["inbounds"][1]["settings"]["autoOutboundsInterface"],
        "auto"
    );
    assert_eq!(generated["metrics"]["listen"], "127.0.0.1:1081");
}
#[test]
fn encrypted_repository_roundtrip() {
    let directory = tempfile::tempdir().unwrap();
    let store = RepositoryStore::new(directory.path()).unwrap();
    let mut repo = Repository::default();
    repo.sources
        .insert("sub".into(), "https://example.com/private-token".into());
    store.save(&repo).unwrap();
    assert_eq!(store.load().unwrap().sources, repo.sources);
    let bytes = std::fs::read(directory.path().join("repository.dpapi")).unwrap();
    assert!(!String::from_utf8_lossy(&bytes).contains("private-token"));
    let encrypted = OsSecretStore.protect(b"secret").unwrap();
    assert_eq!(OsSecretStore.unprotect(&encrypted).unwrap(), b"secret");
}
#[tokio::test]
async fn bundled_core_validation() {
    let directory = tempfile::tempdir().unwrap();
    let path = core::prepare(
        &std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/core"),
        directory.path(),
    )
    .unwrap();
    let version = vpn_service::command(&path)
        .arg("version")
        .output()
        .await
        .unwrap();
    assert!(String::from_utf8_lossy(&version.stdout).contains(core::VERSION));
    let node = uri::parse(VLESS, "sub").unwrap();
    let repo = Repository {
        nodes: vec![node.clone()],
        ..Default::default()
    };
    let bytes =
        serde_json::to_vec(&config::build(&repo, &node.id, 10808, 10809, false).unwrap()).unwrap();
    vpn_service::validate(&path, &bytes).await.unwrap();
    assert!(vpn_service::validate(&path, b"{invalid").await.is_err());
    let advanced = [
        "hysteria2://test-password@example.com:443?sni=example.com".to_owned(),
        "vmess://eyJ2IjoiMiIsImFkZCI6ImV4YW1wbGUuY29tIiwicG9ydCI6NDQzLCJpZCI6IjAwMDAwMDAwLTAwMDAtNDAwMC04MDAwLTAwMDAwMDAwMDAwMSIsIm5ldCI6ImtjcCIsInRscyI6Im5vbmUifQ==".to_owned(),
        serde_json::json!({"protocol":"wireguard","settings":{"secretKey":"AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE=","address":["10.0.0.2/32"],"peers":[{"endpoint":"example.com:51820","publicKey":"AgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgI=","allowedIPs":["0.0.0.0/0"]}]}}).to_string(),
    ];
    for input in advanced {
        let node = parser::parse(&input, "advanced").unwrap().nodes.remove(0);
        let outbound = config::outbound(&node, "proxy").unwrap();
        let roundtrip = parser::parse(&outbound.to_string(), "advanced")
            .unwrap()
            .nodes
            .remove(0);
        assert_eq!(roundtrip.protocol, node.protocol);
        let repo = Repository {
            nodes: vec![node.clone()],
            ..Default::default()
        };
        let bytes =
            serde_json::to_vec(&config::build(&repo, &node.id, 10808, 10809, false).unwrap())
                .unwrap();
        vpn_service::validate(&path, &bytes).await.unwrap();
    }
}

#[cfg(windows)]
#[tokio::test]
async fn core_readiness_does_not_wait_for_external_ip() {
    let directory = tempfile::tempdir().unwrap();
    let path = core::prepare(
        &std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/core"),
        directory.path(),
    )
    .unwrap();
    let (proxy_port, metrics_port) = vpn_service::ports().unwrap();
    let bytes = serde_json::to_vec(&serde_json::json!({
        "log": { "loglevel": "none" },
        "inbounds": [{
            "listen": "127.0.0.1",
            "port": proxy_port,
            "protocol": "socks",
            "settings": { "udp": true }
        }],
        "outbounds": [{ "protocol": "freedom", "tag": "direct" }]
    }))
    .unwrap();
    let started = std::time::Instant::now();
    let mut session = vpn_service::Session::default();
    vpn_service::start(&path, &mut session, &bytes, proxy_port, metrics_port)
        .await
        .unwrap();
    assert!(started.elapsed() < std::time::Duration::from_secs(3));
    vpn_service::stop(&mut session).await.unwrap();
}

#[test]
fn rejects_lossy_translations_and_oversized_inputs() {
    let node = uri::parse(VLESS, "sub").unwrap();
    let mut value = config::outbound(&node, "proxy").unwrap();
    value["streamSettings"]["wsSettings"]["headers"]["Authorization"] =
        serde_json::json!("private");
    assert!(parser::parse(&value.to_string(), "sub").is_err());
    assert!(parser::parse(&"a".repeat(parser::MAX_BYTES + 1), "sub").is_err());
    assert!(parser::parse(&format!("{VLESS}\n").repeat(parser::MAX_NODES + 1), "sub").is_err());
    assert!(parser::parse("proxies:\n - type: trojan\n   password: secret\n   server: example.com\n   port: 443\n   ws-opts: {max-early-data: 2048}","sub").is_err());
}

#[test]
fn ip_rules_and_default_direct() {
    use crate::storage::repository::Rule;
    let node = uri::parse(VLESS, "sub").unwrap();
    let mut repo = Repository {
        nodes: vec![node.clone()],
        routing_mode: "rules".into(),
        ..Default::default()
    };
    for name in [
        "203.0.113.0/24",
        "2001:db8::/32",
        "203.0.113.1",
        "example.com/",
    ] {
        repo.rules = vec![Rule {
            id: "rule".into(),
            name: name.into(),
            is_domain: true,
            server_id: "subscription".into(),
            enabled: true,
        }];
        let value =
            serde_json::to_value(config::build(&repo, &node.id, 10808, 10809, true).unwrap())
                .unwrap();
        let key = if name.starts_with("example") {
            "domain"
        } else {
            "ip"
        };
        assert!(value["routing"]["rules"][3][key].is_array());
    }
    repo.rules[0].name = "203.0.113.0/99".into();
    assert!(config::build(&repo, &node.id, 10808, 10809, true).is_err());
}
#[tokio::test]
#[ignore = "Requires local private subscription and Internet access"]
async fn private_subscription_config_validation() {
    let filename =
        std::env::var_os("LUMEN_TEST_SUBSCRIPTION_FILE").expect("Set local test subscription file");
    let source = std::fs::read_to_string(filename).unwrap();
    let body = crate::subscription::fetcher::fetch(source.trim())
        .await
        .unwrap();
    let parsed = parser::parse(&body, "test").unwrap();
    eprintln!(
        "Supported nodes: {}; unsupported nodes: {}",
        parsed.nodes.len(),
        parsed.diagnostics.len()
    );
    let directory = tempfile::tempdir().unwrap();
    let core = core::prepare(
        &std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/core"),
        directory.path(),
    )
    .unwrap();
    let repo = Repository {
        nodes: parsed.nodes,
        ..Default::default()
    };
    let mut failures = std::collections::BTreeMap::new();
    for node in &repo.nodes {
        let bytes =
            serde_json::to_vec(&config::build(&repo, &node.id, 10808, 10809, false).unwrap())
                .unwrap();
        if let Err(error) = vpn_service::validate(&core, &bytes).await {
            use tokio::io::AsyncWriteExt;
            let mut child = vpn_service::command(&core)
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
    let body = crate::subscription::fetcher::fetch(source.trim())
        .await
        .unwrap();
    let parsed = parser::parse(&body, "smoke").unwrap();
    let directory = tempfile::tempdir().unwrap();
    let core = core::prepare(
        &std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/core"),
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
        vpn_service::validate(&core, &bytes).await.unwrap();
        let mut session = vpn_service::Session::default();
        let result = vpn_service::start(&core, &mut session, &bytes, proxy, metrics).await;
        if result.is_ok() {
            let second = vpn_service::public_ip(proxy).await;
            let counters = crate::services::stats_service::counters(&client, metrics).await;
            vpn_service::stop(&mut session).await.unwrap();
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
        vpn_service::stop(&mut session).await.unwrap();
        eprintln!("Node {index}: unreachable (credentials redacted)");
    }
    assert!(
        connected,
        "No tested node could establish a verified connection"
    );
}

#[test]
fn balanced_xray_subscription_and_clean_names() {
    let sub = r#"[
        {
            "remarks": "🇫🇲 🚀 Автоматический сервер | Европа",
            "outbounds": [
                {
                    "tag": "proxy-1",
                    "protocol": "vless",
                    "settings": {"vnext": [{"address": "example.com", "port": 443, "users": [{"id": "00000000-0000-4000-8000-000000000001"}]}]},
                    "streamSettings": {"network": "tcp", "security": "tls", "tlsSettings": {"serverName": "example.com"}}
                },
                {
                    "tag": "proxy-2",
                    "protocol": "vless",
                    "settings": {"vnext": [{"address": "example.com", "port": 443, "users": [{"id": "00000000-0000-4000-8000-000000000002"}]}]},
                    "streamSettings": {"network": "tcp", "security": "tls", "tlsSettings": {"serverName": "example.com"}}
                },
                {"tag": "direct", "protocol": "freedom"}
            ],
            "routing": {
                "balancers": [{"tag": "auto-eu", "selector": ["proxy"], "strategy": {"type": "leastPing"}}]
            }
        },
        {
            "remarks": "🇫🇮 Финляндия",
            "outbounds": [
                {
                    "tag": "proxy",
                    "protocol": "vless",
                    "settings": {"vnext": [{"address": "example.com", "port": 443, "users": [{"id": "00000000-0000-4000-8000-000000000003"}]}]},
                    "streamSettings": {"network": "tcp", "security": "tls", "tlsSettings": {"serverName": "example.com"}}
                }
            ]
        }
    ]"#;
    let parsed = parser::parse(sub, "sub-test").unwrap();
    assert_eq!(parsed.nodes.len(), 2);
    assert_eq!(parsed.nodes[0].name, "🇫🇲 🚀 Автоматический сервер | Европа");
    assert!(parsed.nodes[0].balancer.is_some());
    let balancer = parsed.nodes[0].balancer.as_ref().unwrap();
    assert_eq!(balancer.tag, "auto-eu");
    assert_eq!(balancer.outbounds.len(), 2);

    assert_eq!(parsed.nodes[1].name, "🇫🇮 Финляндия");
    assert!(parsed.nodes[1].balancer.is_none());

    let mut repo = Repository::default();
    repo.nodes = parsed.nodes;
    let snap = crate::services::catalog_service::snapshot(&repo, &Default::default());
    let servers = snap["servers"].as_array().unwrap();
    assert_eq!(servers[0]["name"], "🚀 Автоматический сервер | Европа");
    assert_eq!(servers[0]["countryCode"], "fm");
    assert_eq!(servers[1]["name"], "Финляндия");
    assert_eq!(servers[1]["countryCode"], "fi");
}
