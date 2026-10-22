use crate::{
    services::vpn_service,
    storage::{
        repository::{Repository, RepositoryStore},
        secure::{OsSecretStore, SecretStore},
    },
    subscription::{parser, uri},
    xray::{bundle, config},
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
fn accepts_plain_vless_subscriptions_from_public_servers() {
    use base64::Engine;

    let link = "vless://00000000-0000-4000-8000-000000000001@203.0.113.10:443?encryption=none&security=none&type=tcp#Plain-VLESS";
    let encoded = base64::engine::general_purpose::STANDARD.encode(link);
    let parsed = parser::parse(&encoded, "plain-vless").unwrap();

    assert_eq!(parsed.nodes.len(), 1);
    assert!(parsed.diagnostics.is_empty());
    assert_eq!(parsed.nodes[0].security.kind, "none");
    assert_eq!(parsed.nodes[0].transport.network, "tcp");
    assert!(config::outbound(&parsed.nodes[0], "proxy").is_ok());
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
fn discord_uses_the_vpn_and_other_traffic_stays_direct() {
    use crate::services::catalog_service::generate_preset_rules;
    let node = uri::parse(VLESS, "sub").unwrap();
    let mut repo = Repository {
        nodes: vec![node.clone()],
        routing_mode: "rules".into(),
        ..Default::default()
    };
    repo.rules = generate_preset_rules("russia-optimized", "subscription").unwrap();
    let config =
        serde_json::to_value(config::build(&repo, &node.id, 10808, 10809, true).unwrap()).unwrap();
    let rules = config["routing"]["rules"].as_array().unwrap();
    let discord = rules
        .iter()
        .position(|rule| rule["process"][0] == "Discord.exe")
        .unwrap();
    let category_ru = rules
        .iter()
        .position(|rule| rule["domain"][0] == "geosite:category-ru")
        .unwrap();
    // Discord voice servers are bare addresses: a Russian GeoIP match ahead of the
    // process rule would send them to the blocked direct path.
    let geoip_ru = rules
        .iter()
        .position(|rule| rule["ip"][0] == "geoip:ru")
        .unwrap();
    assert!(discord < category_ru && discord < geoip_ru);
    assert_eq!(rules[discord]["outboundTag"], "proxy");
    assert!(rules
        .iter()
        .any(|rule| rule["domain"][0] == "geosite:discord" && rule["outboundTag"] == "proxy"));
    assert_eq!(rules.last().unwrap()["outboundTag"], "direct");
    assert!(rules.iter().all(|rule| rule["network"] != "udp"));
}
#[test]
fn discord_media_leaves_a_server_that_drops_it() {
    use crate::services::catalog_service::generate_preset_rules;
    let node = uri::parse(VLESS, "sub").unwrap();
    let fallback = uri::parse(
        "vless://00000000-0000-4000-8000-000000000002@fallback.example.com:443?type=tcp&security=tls&sni=fallback.example.com#Fallback",
        "sub",
    )
    .unwrap();
    let mut repo = Repository {
        nodes: vec![node.clone(), fallback.clone()],
        routing_mode: "rules".into(),
        ..Default::default()
    };
    repo.rules = generate_preset_rules("russia-optimized", "subscription").unwrap();
    let config_of = |repo: &Repository| {
        serde_json::to_value(config::build(repo, &node.id, 10808, 10809, true).unwrap()).unwrap()
    };
    let is_voice =
        |rule: &&serde_json::Value| rule["network"] == "udp" && rule["process"].is_array();
    let voice_of = |config: &serde_json::Value| {
        config["routing"]["rules"]
            .as_array()
            .unwrap()
            .iter()
            .find(is_voice)
            .cloned()
    };
    assert!(voice_of(&config_of(&repo)).is_none());

    // Another server carries Discord media: its outbound joins the config.
    let before = repo.clone();
    repo.discord_voice_routes
        .insert(node.id.clone(), fallback.id.clone());
    let config = config_of(&repo);
    let rules = config["routing"]["rules"].as_array().unwrap();
    let voice = rules.iter().position(|rule| is_voice(&rule)).unwrap();
    let discord = rules
        .iter()
        .position(|rule| rule["process"][0] == "Discord.exe" && rule["network"].is_null())
        .unwrap();
    assert!(voice < discord);
    assert_eq!(rules[voice]["outboundTag"], fallback.id.as_str());
    assert_eq!(rules[voice]["port"], "19294-19344,50000-50100");
    for name in [
        "Discord.exe",
        "discord.exe",
        "DiscordPTB.exe",
        "DiscordCanary.exe",
    ] {
        assert!(rules[voice]["process"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == name));
    }
    assert!(config["outbounds"]
        .as_array()
        .unwrap()
        .iter()
        .any(|outbound| outbound["tag"] == fallback.id.as_str()));
    // The probe inbound keeps measuring the selected exit itself.
    let probe = rules
        .iter()
        .position(|rule| rule["inboundTag"][0] == "probe")
        .unwrap();
    assert!(probe < voice);
    #[cfg(any(windows, target_os = "macos"))]
    {
        let plan = config::live_routing_plan(&before, &repo, &node.id, true)
            .unwrap()
            .unwrap();
        assert_eq!(plan.added_outbounds.len(), 1);
        assert_eq!(plan.added_outbounds[0]["tag"], fallback.id.as_str());
    }

    let mut direct = repo.clone();
    direct
        .discord_voice_routes
        .insert(node.id.clone(), "direct".into());
    assert_eq!(
        voice_of(&config_of(&direct)).unwrap()["outboundTag"],
        "direct"
    );

    // A fallback that left the subscription degrades to the direct path.
    let mut missing = repo.clone();
    missing
        .discord_voice_routes
        .insert(node.id.clone(), "gone".into());
    assert_eq!(
        voice_of(&config_of(&missing)).unwrap()["outboundTag"],
        "direct"
    );

    let mut global = repo.clone();
    global.routing_mode = "global".into();
    assert!(voice_of(&config_of(&global)).is_some());

    let mut other = repo.clone();
    other.discord_voice_routes = [("another-server".to_string(), "direct".to_string())].into();
    assert!(voice_of(&config_of(&other)).is_none());

    let mut blocked = repo.clone();
    blocked.rules.push(crate::storage::repository::Rule {
        id: "block-discord".into(),
        name: "C:\\Apps\\discord.exe".into(),
        matcher: crate::storage::repository::RuleMatcher::Application,
        server_id: "block".into(),
        enabled: true,
        preset_id: None,
        is_domain: false,
    });
    assert!(voice_of(&config_of(&blocked)).is_none());
}
#[test]
fn subscription_routing_from_clash_and_incy() {
    use base64::Engine;
    let yaml = r#"proxies:
  - name: Poland
    type: trojan
    server: example.com
    port: 443
    password: secret
rules:
  - GEOSITE,CATEGORY-RU,DIRECT
  - GEOIP,RU,DIRECT
  - PROCESS-NAME,Discord.exe,Poland
  - DOMAIN-SUFFIX,openai.com,Poland
  - MATCH,Poland
"#;
    let parsed = parser::parse(yaml, "sub").unwrap();
    assert_eq!(parsed.nodes.len(), 1);
    let names: Vec<_> = parsed.rules.iter().map(|rule| rule.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["category-ru", "ru", "Discord.exe", "openai.com", "match"]
    );
    assert_eq!(parsed.rules[0].server_id, "direct");
    assert_eq!(parsed.rules[2].server_id, parsed.nodes[0].id);
    assert_eq!(
        parsed.rules[4].matcher,
        crate::storage::repository::RuleMatcher::Final
    );
    assert_eq!(parsed.rules[4].server_id, parsed.nodes[0].id);

    let profile = r#"{"Name":"Example VPN","GlobalProxy":"true","DirectSites":["geosite:category-ru"],"DirectIp":["geoip:ru"],"BlockSites":["geosite:category-ads-all"],"ProxySites":["example.org"]}"#;
    let link = format!(
        "incy://routing/onadd/{}",
        base64::engine::general_purpose::STANDARD.encode(profile)
    );
    let body = format!("{VLESS}\n{link}");
    let parsed = parser::parse(&body, "provider").unwrap();
    assert_eq!(parsed.nodes.len(), 1);
    assert!(parsed.diagnostics.is_empty());
    assert!(parsed
        .rules
        .iter()
        .any(|rule| rule.name == "category-ru" && rule.server_id == "direct"));
    assert!(parsed
        .rules
        .iter()
        .any(|rule| rule.name == "category-ads-all" && rule.server_id == "block"));
    assert!(parsed.rules.iter().any(|rule| rule.name == "example.org"));
    assert_eq!(
        parsed.rules.last().unwrap().matcher,
        crate::storage::repository::RuleMatcher::Final
    );
    assert_eq!(parsed.rules.last().unwrap().server_id, "subscription");

    let off = format!("{VLESS}\nincy://routing/off");
    let parsed = parser::parse(&off, "provider").unwrap();
    assert!(parsed.routing_off);
    assert!(parsed.rules.is_empty());
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
    let runtime = crate::runtime::RuntimePaths::new(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("runtime"),
    );
    let path = bundle::prepare(&runtime, directory.path()).unwrap();
    let version = crate::engine::xray::command(&path)
        .arg("version")
        .output()
        .await
        .unwrap();
    assert!(String::from_utf8_lossy(&version.stdout).contains(bundle::VERSION));
    let node = uri::parse(VLESS, "sub").unwrap();
    let repo = Repository {
        nodes: vec![node.clone()],
        ..Default::default()
    };
    let bytes =
        serde_json::to_vec(&config::build(&repo, &node.id, 10808, 10809, false).unwrap()).unwrap();
    crate::engine::xray::validate(&path, &bytes).await.unwrap();
    #[cfg(windows)]
    {
        let standby = config::build_standby(&repo, &node.id, 10808, 10809).unwrap();
        let value = serde_json::to_value(&standby).unwrap();
        let tun = value["inbounds"]
            .as_array()
            .unwrap()
            .iter()
            .find(|inbound| inbound["protocol"] == "tun")
            .unwrap();
        assert!(tun["settings"].get("autoSystemRoutingTable").is_none());
        crate::engine::xray::validate(&path, &serde_json::to_vec(&standby).unwrap())
            .await
            .unwrap();
    }
    assert!(crate::engine::xray::validate(&path, b"{invalid")
        .await
        .is_err());
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
        crate::engine::xray::validate(&path, &bytes).await.unwrap();
    }
}

#[cfg(any(windows, target_os = "macos"))]
#[tokio::test]
async fn core_readiness_does_not_wait_for_external_ip() {
    let directory = tempfile::tempdir().unwrap();
    let runtime = crate::runtime::RuntimePaths::new(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("runtime"),
    );
    let path = bundle::prepare(&runtime, directory.path()).unwrap();
    let (proxy_port, metrics_port) = crate::engine::xray::ports().unwrap();
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
    let mut samples = Vec::new();
    for _ in 0..9 {
        let started = std::time::Instant::now();
        let mut session = crate::engine::xray::XraySession::default();
        crate::engine::xray::start(&path, &mut session, &bytes, proxy_port, metrics_port)
            .await
            .unwrap();
        samples.push(started.elapsed().as_millis());
        crate::engine::xray::stop(&mut session).await.unwrap();
    }
    samples.sort_unstable();
    eprintln!(
        "Local Xray process + SOCKS readiness: median={} ms, p95={} ms (n=9)",
        samples[4], samples[8]
    );
    assert!(samples[8] < 3000);
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
    use crate::storage::repository::{Rule, RuleMatcher};
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
            matcher: RuleMatcher::Domain,
            server_id: "subscription".into(),
            enabled: true,
            preset_id: None,
            is_domain: true,
        }];
        let value =
            serde_json::to_value(config::build(&repo, &node.id, 10808, 10809, true).unwrap())
                .unwrap();
        let key = if name.starts_with("example") {
            "domain"
        } else {
            "ip"
        };
        assert!(value["routing"]["rules"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r[key].is_array() && r["outboundTag"] == "proxy"));
    }
    repo.rules[0].name = "203.0.113.0/99".into();
    assert!(config::build(&repo, &node.id, 10808, 10809, true).is_err());
}
#[cfg(any(windows, target_os = "macos"))]
#[test]
fn disabling_a_rule_can_replace_live_routing_without_restarting_tun() {
    use crate::storage::repository::{Rule, RuleMatcher};
    let node = uri::parse(VLESS, "sub").unwrap();
    let mut before = Repository {
        nodes: vec![node.clone()],
        routing_mode: "rules".into(),
        ..Default::default()
    };
    before.rules.push(Rule {
        id: "steam".into(),
        name: "steam.exe".into(),
        matcher: RuleMatcher::Application,
        server_id: "subscription".into(),
        enabled: true,
        preset_id: None,
        is_domain: false,
    });
    let mut after = before.clone();
    after.rules[0].enabled = false;
    let patch = config::live_routing_patch(&before, &after, &node.id, true)
        .unwrap()
        .unwrap();
    assert!(!patch["routing"]["rules"]
        .as_array()
        .unwrap()
        .iter()
        .any(|rule| rule["process"]
            .as_array()
            .is_some_and(|processes| processes.iter().any(|process| process == "steam.exe"))));
    assert!(config::live_routing_patch(&after, &before, &node.id, true)
        .unwrap()
        .is_some());
    let mut removed = before.clone();
    removed.rules.clear();
    assert!(
        config::live_routing_patch(&before, &removed, &node.id, true)
            .unwrap()
            .is_some()
    );
}
#[cfg(windows)]
#[test]
fn tun_captures_ipv6_only_when_the_physical_network_has_it() {
    let node = uri::parse(VLESS, "sub").unwrap();
    let repo = Repository {
        nodes: vec![node.clone()],
        ..Default::default()
    };
    let config =
        serde_json::to_value(config::build(&repo, &node.id, 10808, 10809, true).unwrap()).unwrap();
    let tun = config["inbounds"]
        .as_array()
        .unwrap()
        .iter()
        .find(|inbound| inbound["protocol"] == "tun")
        .unwrap();
    let routes = tun["settings"]["autoSystemRoutingTable"]
        .as_array()
        .unwrap();
    assert_eq!(routes[0], "0.0.0.0/0");
    assert_eq!(
        routes.iter().any(|route| route == "::/0"),
        crate::platform::network::physical_ipv6_available()
    );
}
#[test]
fn vpn_applications_do_not_override_russian_direct_presets() {
    use crate::services::catalog_service::generate_preset_rules;
    use crate::storage::repository::{Rule, RuleMatcher};
    let node = uri::parse(VLESS, "sub").unwrap();
    let app = |id: &str, server_id: &str| Rule {
        id: id.into(),
        name: format!("{id}.exe"),
        matcher: RuleMatcher::Application,
        server_id: server_id.into(),
        enabled: true,
        preset_id: None,
        is_domain: false,
    };
    let mut repo = Repository {
        nodes: vec![node.clone()],
        routing_mode: "rules".into(),
        ..Default::default()
    };
    repo.rules = vec![app("chrome", "subscription"), app("torrent", "direct")];
    repo.rules
        .extend(generate_preset_rules("russia-optimized", "subscription").unwrap());
    let config =
        serde_json::to_value(config::build(&repo, &node.id, 10808, 10809, true).unwrap()).unwrap();
    let rules = config["routing"]["rules"].as_array().unwrap();
    let position = |f: &dyn Fn(&serde_json::Value) -> bool| rules.iter().position(f).unwrap();
    let process =
        |name: &'static str| move |r: &serde_json::Value| r["process"][0].as_str() == Some(name);
    let chrome = position(&process("chrome.exe"));
    let torrent = position(&process("torrent.exe"));
    let category_ru = position(&|r| r["domain"][0] == "geosite:category-ru");
    let geoip_ru = position(&|r| r["ip"][0] == "geoip:ru");
    assert!(category_ru < chrome && geoip_ru < chrome);
    assert!(torrent < category_ru, "direct applications stay absolute");
    assert_eq!(rules[chrome]["outboundTag"], "proxy");
    let catch_all = rules.last().unwrap();
    assert_eq!(catch_all["ip"], serde_json::json!(["0.0.0.0/0", "::/0"]));
    assert_eq!(catch_all["outboundTag"], "direct");
    assert!(position(&process("Discord.exe")) < chrome);
}
#[cfg(any(windows, target_os = "macos"))]
#[test]
fn switching_routing_modes_keeps_the_active_tun_configuration() {
    use crate::storage::repository::{Rule, RuleMatcher};
    let node = uri::parse(VLESS, "sub").unwrap();
    let mut rules = Repository {
        nodes: vec![node.clone()],
        routing_mode: "rules".into(),
        ..Default::default()
    };
    rules.rules.push(Rule {
        id: "steam".into(),
        name: "steam.exe".into(),
        matcher: RuleMatcher::Application,
        server_id: "subscription".into(),
        enabled: true,
        preset_id: None,
        is_domain: false,
    });
    let mut global = rules.clone();
    global.routing_mode = "global".into();
    let rules_config =
        serde_json::to_value(config::build(&rules, &node.id, 10808, 10809, true).unwrap()).unwrap();
    let global_config =
        serde_json::to_value(config::build(&global, &node.id, 10808, 10809, true).unwrap())
            .unwrap();
    for field in ["inbounds", "outbounds", "dns"] {
        assert_eq!(
            rules_config[field], global_config[field],
            "{field} changed across modes"
        );
    }
    assert_eq!(
        rules_config["routing"]["domainStrategy"],
        global_config["routing"]["domainStrategy"]
    );
    assert_eq!(global_config["routing"]["domainStrategy"], "IPIfNonMatch");
    assert_eq!(
        global_config["routing"]["rules"][0]["inboundTag"][0],
        "dns-query"
    );
    assert!(config::live_routing_patch(&rules, &global, &node.id, true)
        .unwrap()
        .is_some());
    assert!(config::live_routing_patch(&global, &rules, &node.id, true)
        .unwrap()
        .is_some());
    assert_eq!(
        global_config["routing"]["rules"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()["outboundTag"],
        "proxy"
    );
    assert_eq!(
        rules_config["routing"]["rules"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()["outboundTag"],
        "direct"
    );
}
#[cfg(any(windows, target_os = "macos"))]
#[test]
fn switching_routing_modes_keeps_the_system_proxy_configuration() {
    let node = uri::parse(VLESS, "sub").unwrap();
    let mut rules = Repository {
        nodes: vec![node.clone()],
        routing_mode: "rules".into(),
        ..Default::default()
    };
    rules.settings.network_mode = "proxy".into();
    rules.settings.dns_server = "1.1.1.1".into();
    let mut global = rules.clone();
    global.routing_mode = "global".into();
    let rules_config =
        serde_json::to_value(config::build(&rules, &node.id, 10808, 10809, false).unwrap())
            .unwrap();
    let global_config =
        serde_json::to_value(config::build(&global, &node.id, 10808, 10809, false).unwrap())
            .unwrap();
    for field in ["inbounds", "outbounds", "dns"] {
        assert_eq!(
            rules_config[field], global_config[field],
            "{field} changed across modes"
        );
    }
    assert_eq!(
        rules_config["routing"]["domainStrategy"],
        global_config["routing"]["domainStrategy"]
    );
    assert!(config::live_routing_patch(&rules, &global, &node.id, false)
        .unwrap()
        .is_some());
    assert!(config::live_routing_patch(&global, &rules, &node.id, false)
        .unwrap()
        .is_some());
}
#[cfg(any(windows, target_os = "macos"))]
#[test]
fn dns_policy_keeps_mode_and_geo_rules_on_the_live_path() {
    use crate::storage::repository::{Rule, RuleMatcher};
    let node = uri::parse(VLESS, "sub").unwrap();
    let mut rules = Repository {
        nodes: vec![node.clone()],
        routing_mode: "rules".into(),
        ..Default::default()
    };
    rules.settings.dns_policy.enabled = true;
    rules.settings.dns_policy.direct = "9.9.9.9".into();
    rules.rules.extend([
        Rule {
            id: "geo-site".into(),
            name: "antifilter".into(),
            matcher: RuleMatcher::Geosite,
            server_id: "subscription".into(),
            enabled: true,
            preset_id: None,
            is_domain: false,
        },
        Rule {
            id: "geo-ip".into(),
            name: "ru".into(),
            matcher: RuleMatcher::Geoip,
            server_id: "direct".into(),
            enabled: true,
            preset_id: None,
            is_domain: false,
        },
    ]);
    let mut global = rules.clone();
    global.routing_mode = "global".into();
    let rules_config =
        serde_json::to_value(config::build(&rules, &node.id, 10808, 10809, true).unwrap()).unwrap();
    let global_config =
        serde_json::to_value(config::build(&global, &node.id, 10808, 10809, true).unwrap())
            .unwrap();
    for field in ["inbounds", "outbounds", "dns"] {
        assert_eq!(
            rules_config[field], global_config[field],
            "{field} changed across modes"
        );
    }
    assert!(config::live_routing_patch(&rules, &global, &node.id, true)
        .unwrap()
        .is_some());
    assert!(config::live_routing_patch(&global, &rules, &node.id, true)
        .unwrap()
        .is_some());
    let mut added = rules.clone();
    added.rules.push(Rule {
        id: "extra-site".into(),
        name: "google".into(),
        matcher: RuleMatcher::Geosite,
        server_id: "subscription".into(),
        enabled: true,
        preset_id: None,
        is_domain: false,
    });
    let added_config =
        serde_json::to_value(config::build(&added, &node.id, 10808, 10809, true).unwrap()).unwrap();
    assert_eq!(rules_config["dns"], added_config["dns"]);
    assert_eq!(rules_config["inbounds"], added_config["inbounds"]);
    assert_eq!(rules_config["outbounds"], added_config["outbounds"]);
    assert!(config::live_routing_patch(&rules, &added, &node.id, true)
        .unwrap()
        .is_some());
}
#[cfg(any(windows, target_os = "macos"))]
#[test]
fn rule_for_another_server_is_a_live_outbound_and_route() {
    use crate::storage::repository::{Rule, RuleMatcher};
    let first = uri::parse(VLESS, "sub").unwrap();
    let second = uri::parse(
        "vless://00000000-0000-4000-8000-000000000002@b.example:443?security=tls#B",
        "sub",
    )
    .unwrap();
    let mut before = Repository {
        nodes: vec![first.clone(), second.clone()],
        routing_mode: "rules".into(),
        ..Default::default()
    };
    before.settings.dns_server = "1.1.1.1".into();
    before.settings.dns_policy.enabled = true;
    before.settings.dns_policy.direct = "9.9.9.9".into();
    let mut after = before.clone();
    after.rules.push(Rule {
        id: "other".into(),
        name: "example.org".into(),
        matcher: RuleMatcher::Domain,
        server_id: second.id.clone(),
        enabled: true,
        preset_id: None,
        is_domain: true,
    });
    let before_config =
        serde_json::to_value(config::build(&before, &first.id, 10808, 10809, true).unwrap())
            .unwrap();
    let after_config =
        serde_json::to_value(config::build(&after, &first.id, 10808, 10809, true).unwrap())
            .unwrap();
    assert_eq!(before_config["dns"], after_config["dns"]);
    assert_eq!(before_config["inbounds"], after_config["inbounds"]);
    let plan = config::live_routing_plan(&before, &after, &first.id, true)
        .unwrap()
        .unwrap();
    assert_eq!(plan.added_outbounds.len(), 1);
    assert_eq!(plan.added_outbounds[0]["tag"], second.id);
    assert!(plan.patch["routing"]["rules"]
        .as_array()
        .unwrap()
        .iter()
        .any(|rule| rule["outboundTag"] == second.id && rule["domain"][0] == "domain:example.org"));
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

    let repo = Repository {
        nodes: parsed.nodes,
        ..Default::default()
    };
    let snap = crate::services::catalog_service::snapshot(&repo, &Default::default());
    let servers = snap["servers"].as_array().unwrap();
    assert_eq!(servers[0]["name"], "🚀 Автоматический сервер | Европа");
    assert_eq!(servers[0]["countryCode"], "fm");
    assert_eq!(servers[1]["name"], "Финляндия");
    assert_eq!(servers[1]["countryCode"], "fi");
}

#[test]
fn rule_model_backward_compatibility_migration() {
    use crate::storage::repository::{Rule, RuleMatcher};

    // Old rule with isDomain: true should deserialize to RuleMatcher::Domain
    let legacy_domain = r#"{
        "id": "leg-dom",
        "name": "example.com",
        "isDomain": true,
        "serverId": "direct",
        "enabled": true
    }"#;
    let rule: Rule = serde_json::from_str(legacy_domain).unwrap();
    assert_eq!(rule.matcher, RuleMatcher::Domain);
    assert!(rule.is_domain);
    assert_eq!(rule.id, "leg-dom");

    // Old rule with isDomain: false should deserialize to RuleMatcher::Application
    let legacy_app = r#"{
        "id": "leg-app",
        "name": "discord.exe",
        "isDomain": false,
        "serverId": "subscription",
        "enabled": true
    }"#;
    let rule: Rule = serde_json::from_str(legacy_app).unwrap();
    assert_eq!(rule.matcher, RuleMatcher::Application);
    assert!(!rule.is_domain);

    // New rule with explicit matcher geosite
    let geosite_rule = r#"{
        "id": "geo-s",
        "name": "category-ru",
        "matcher": "geosite",
        "serverId": "direct",
        "enabled": true
    }"#;
    let rule: Rule = serde_json::from_str(geosite_rule).unwrap();
    assert_eq!(rule.matcher, RuleMatcher::Geosite);
    assert!(!rule.is_domain);

    // New rule with explicit matcher geoip
    let geoip_rule = r#"{
        "id": "geo-i",
        "name": "ru",
        "matcher": "geoip",
        "serverId": "direct",
        "enabled": true
    }"#;
    let rule: Rule = serde_json::from_str(geoip_rule).unwrap();
    assert_eq!(rule.matcher, RuleMatcher::Geoip);

    // Serialization must contain both matcher and isDomain for frontend/mock compatibility
    let serialized = serde_json::to_value(&rule).unwrap();
    assert_eq!(serialized["matcher"], "geoip");
    assert_eq!(serialized["isDomain"], false);
}

#[test]
fn geo_rules_xray_config_compilation() {
    use crate::storage::repository::{Rule, RuleMatcher};

    let node = uri::parse(VLESS, "sub").unwrap();
    let mut repo = Repository {
        nodes: vec![node.clone()],
        routing_mode: "rules".into(),
        ..Default::default()
    };

    repo.rules = vec![
        Rule {
            id: "r1".into(),
            name: "private".into(),
            matcher: RuleMatcher::Geoip,
            server_id: "direct".into(),
            enabled: true,
            preset_id: None,
            is_domain: false,
        },
        Rule {
            id: "r2".into(),
            name: "geosite:antifilter".into(), // prefix should be stripped
            matcher: RuleMatcher::Geosite,
            server_id: "subscription".into(),
            enabled: true,
            preset_id: None,
            is_domain: false,
        },
        Rule {
            id: "r3".into(),
            name: "category-ru".into(),
            matcher: RuleMatcher::Geosite,
            server_id: "block".into(),
            enabled: true,
            preset_id: None,
            is_domain: false,
        },
        Rule {
            id: "r4".into(),
            name: "geoip:ru".into(), // prefix should be stripped
            matcher: RuleMatcher::Geoip,
            server_id: "direct".into(),
            enabled: true,
            preset_id: None,
            is_domain: false,
        },
    ];

    let value =
        serde_json::to_value(config::build(&repo, &node.id, 10808, 10809, true).unwrap()).unwrap();

    assert_eq!(value["routing"]["domainStrategy"], "IPIfNonMatch");

    let rules = value["routing"]["rules"].as_array().unwrap();
    let rule =
        |pred: &dyn Fn(&serde_json::Value) -> bool| rules.iter().find(|rule| pred(rule)).unwrap();

    let private = rule(&|rule| rule["ip"][0] == "geoip:private");
    assert!(private.get("domain").is_none());
    assert_eq!(private["outboundTag"], "direct");

    let antifilter = rule(&|rule| rule["domain"][0] == "geosite:antifilter");
    assert!(antifilter.get("ip").is_none());
    assert_eq!(antifilter["outboundTag"], "proxy");

    let category_ru = rule(&|rule| rule["domain"][0] == "geosite:category-ru");
    assert_eq!(category_ru["outboundTag"], "block");

    let geoip_ru = rule(&|rule| rule["ip"][0] == "geoip:ru");
    assert_eq!(geoip_ru["outboundTag"], "direct");

    // Final rules: unhandled DNS (port 53) and default fallback
    // An address catch-all, so IP rules still get the resolved-domain pass.
    let last = rules.last().unwrap();
    assert_eq!(last["ip"], serde_json::json!(["0.0.0.0/0", "::/0"]));
    assert!(last.get("network").is_none());
    assert_eq!(last["outboundTag"], "direct");

    // Tag validation: mismatched prefix must error
    repo.rules = vec![Rule {
        id: "bad".into(),
        name: "geosite:category-ru".into(),
        matcher: RuleMatcher::Geoip,
        server_id: "direct".into(),
        enabled: true,
        preset_id: None,
        is_domain: false,
    }];
    assert!(config::build(&repo, &node.id, 10808, 10809, true).is_err());

    // Tag validation: invalid characters like path traversal / ext: must error
    repo.rules = vec![Rule {
        id: "bad2".into(),
        name: "ext:bad.dat:ru".into(),
        matcher: RuleMatcher::Geosite,
        server_id: "direct".into(),
        enabled: true,
        preset_id: None,
        is_domain: false,
    }];
    assert!(config::build(&repo, &node.id, 10808, 10809, true).is_err());
}

#[test]
fn preset_application_idempotency_and_preservation() {
    use crate::services::catalog_service::generate_preset_rules;

    let rules = generate_preset_rules("russia-optimized", "sub-1").unwrap();
    assert_eq!(rules.len(), 12);
    assert_eq!(
        rules.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
        vec![
            "Discord.exe",
            "private",
            "category-ru",
            "ru",
            "antifilter",
            "antifilter-community",
            "discord",
            "telegram",
            "youtube",
            "antifilter",
            "antifilter-community",
            "telegram"
        ]
    );
    assert_eq!(rules[6].server_id, "sub-1");
    assert_eq!(
        rules
            .iter()
            .find(|rule| rule.name == "Discord.exe")
            .unwrap()
            .matcher,
        crate::storage::repository::RuleMatcher::Application
    );
    // Check all preset rules have preset_id populated
    for r in &rules {
        assert_eq!(r.preset_id.as_deref(), Some("russia-optimized"));
    }

    // Check bypass-blocking
    let bb = generate_preset_rules("bypass-blocking", "sub-1").unwrap();
    assert_eq!(bb.len(), 8);
    assert_eq!(bb[0].name, "antifilter");
    let rd = generate_preset_rules("russia-direct", "sub-1").unwrap();
    assert_eq!(
        rd.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
        vec!["private", "category-ru", "ru"]
    );

    // Check case-insensitive prefix normalization
    assert_eq!(
        crate::storage::repository::normalize_geo_tag("GeoSite:category-ru", "geosite").unwrap(),
        "category-ru"
    );
    assert_eq!(
        crate::storage::repository::normalize_geo_tag("GEOSITE:antifilter", "geosite").unwrap(),
        "antifilter"
    );
    assert_eq!(
        crate::storage::repository::normalize_geo_tag("GeoIP:ru", "geoip").unwrap(),
        "ru"
    );
    assert_eq!(
        crate::storage::repository::normalize_geo_tag("GEOIP:private", "geoip").unwrap(),
        "private"
    );

    // Test frontend IPC deserialization of Mutation::ApplyPreset and Mutation::RemovePreset
    let json_str = r#"{"kind":"applyPreset","presetId":"russia-optimized"}"#;
    let mutation: crate::services::catalog_service::Mutation =
        serde_json::from_str(json_str).unwrap();
    match mutation {
        crate::services::catalog_service::Mutation::ApplyPreset { preset_id } => {
            assert_eq!(preset_id, "russia-optimized");
        }
        _ => panic!("Expected ApplyPreset"),
    }

    let json_rm = r#"{"kind":"removePreset","presetId":"russia-optimized"}"#;
    let mutation_rm: crate::services::catalog_service::Mutation =
        serde_json::from_str(json_rm).unwrap();
    match mutation_rm {
        crate::services::catalog_service::Mutation::RemovePreset { preset_id } => {
            assert_eq!(preset_id, "russia-optimized");
        }
        _ => panic!("Expected RemovePreset"),
    }
}

#[tokio::test]
async fn geo_presets_real_xray_outbound_selection_and_dns_priority() {
    use crate::services::catalog_service::{generate_preset_rules, ordered_rules};
    use crate::storage::repository::{Rule, RuleMatcher};
    use tokio::io::AsyncWriteExt;
    let node = uri::parse(VLESS, "sub").unwrap();
    let mut repo = Repository {
        nodes: vec![node.clone()],
        routing_mode: "rules".into(),
        ..Default::default()
    };
    let manual = Rule {
        id: "manual".into(),
        name: "203.0.113.10".into(),
        matcher: RuleMatcher::Domain,
        server_id: "block".into(),
        enabled: true,
        preset_id: None,
        is_domain: true,
    };
    let mut expected = None;
    for order in [
        ["russia-direct", "bypass-blocking"],
        ["bypass-blocking", "russia-direct"],
    ] {
        repo.rules = vec![manual.clone()];
        for preset in order {
            repo.rules
                .extend(generate_preset_rules(preset, "subscription").unwrap());
        }
        let ordered: Vec<_> = ordered_rules(&repo.rules).into_iter().cloned().collect();
        if let Some(expected) = &expected {
            assert_eq!(&ordered, expected);
        } else {
            expected = Some(ordered);
        }
    }
    let runtime = crate::runtime::RuntimePaths::new(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("runtime"),
    );
    let data = tempfile::tempdir().unwrap();
    let core = bundle::prepare(&runtime, data.path()).unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let mut config =
        serde_json::to_value(config::build(&repo, &node.id, 10808, 10809, false).unwrap()).unwrap();
    let rules = config["routing"]["rules"].as_array().unwrap();
    assert!(
        rules
            .iter()
            .position(|r| r["outboundTag"] == "dns")
            .unwrap()
            < rules.iter().position(|r| r["ip"].is_array()).unwrap()
    );
    assert_eq!(rules[0]["inboundTag"][0], "dns-query");
    crate::engine::xray::validate(&core, &serde_json::to_vec(&config).unwrap())
        .await
        .unwrap();
    let tun = config::build(&repo, &node.id, 10808, 10809, true).unwrap();
    crate::engine::xray::validate(&core, &serde_json::to_vec(&tun).unwrap())
        .await
        .unwrap();
    // Exercise the real router, with deterministic DNS and inert outbounds; no private node required.
    config["log"] = serde_json::json!({"loglevel":"info"});
    config["inbounds"] = serde_json::json!([{"tag":"diagnostic","protocol":"http","listen":"127.0.0.1","port":port}]);
    config["dns"] = serde_json::json!({"hosts":{
        "ip-only.test":"203.0.113.10", "www.youtube.com":"77.88.55.77",
        "rr1.googlevideo.com":"77.88.55.77", "youtubei.googleapis.com":"77.88.55.77",
        "i.ytimg.com":"77.88.55.77", "yandex.ru":"77.88.55.77", "fallback.test":"203.0.113.20"
    }});
    config["outbounds"] = serde_json::json!([
        {"tag":"direct","protocol":"blackhole"}, {"tag":"proxy","protocol":"blackhole"},
        {"tag":"block","protocol":"blackhole"}, {"tag":"dns","protocol":"blackhole"}
    ]);
    config.as_object_mut().unwrap().remove("metrics");
    let output = tempfile::NamedTempFile::new().unwrap();
    drop(listener);
    let mut child = crate::engine::xray::command(&core)
        .args(["run", "-config", "stdin:"])
        .stdin(std::process::Stdio::piped())
        .stdout(output.as_file().try_clone().unwrap())
        .stderr(output.as_file().try_clone().unwrap())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&serde_json::to_vec(&config).unwrap())
        .await
        .unwrap();
    for _ in 0..100 {
        if tokio::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port))
            .await
            .is_ok()
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    let client = reqwest::Client::builder()
        .no_proxy()
        .proxy(reqwest::Proxy::all(format!("http://127.0.0.1:{port}")).unwrap())
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap();
    let cases = [
        ("ip-only.test", "block"),
        ("www.youtube.com", "proxy"),
        ("rr1.googlevideo.com", "proxy"),
        ("youtubei.googleapis.com", "proxy"),
        ("i.ytimg.com", "proxy"),
        ("yandex.ru", "direct"),
        ("fallback.test", "direct"),
    ];
    for (host, _) in cases {
        let _ = client.get(format!("http://{host}/")).send().await;
    }
    // Xray writes access logs asynchronously, after a blackhole can close the
    // HTTP request. Wait for every route before terminating the process.
    for _ in 0..100 {
        let logs = String::from_utf8_lossy(&std::fs::read(output.path()).unwrap()).into_owned();
        if cases.iter().all(|(host, target)| {
            logs.contains(&format!("taking detour [{target}] for [tcp:{host}:80]"))
        }) {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    child.kill().await.unwrap();
    child.wait().await.unwrap();
    let logs = std::fs::read_to_string(output.path()).unwrap();
    for (host, target) in cases {
        assert!(
            logs.contains(&format!("taking detour [{target}] for [tcp:{host}:80]")),
            "No expected route for {host}; Xray logs:\n{logs}"
        );
    }
    // Compiling old saved presets must not recreate a deleted service row.
    repo.rules.retain(|r| r.name != "youtube");
    let value =
        serde_json::to_value(config::build(&repo, &node.id, 10808, 10809, false).unwrap()).unwrap();
    assert!(!value.to_string().contains("geosite:youtube"));
}

#[tokio::test]
async fn geo_database_delivery_and_initialization() {
    let runtime = crate::runtime::RuntimePaths::new(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("runtime"),
    );
    let temp_data = tempfile::tempdir().unwrap();

    // Verify initial state is missing
    let info = crate::xray::geo::info(temp_data.path());
    assert_eq!(info.status, "missing");

    // Initialize from bundled resources
    crate::xray::geo::ensure_initialized(&runtime, temp_data.path()).unwrap();

    // Verify initialized status is ready
    let info = crate::xray::geo::info(temp_data.path());
    assert_eq!(info.status, "ready");
    assert_eq!(info.version, crate::xray::geo::BUNDLED_GEO_VERSION);
    assert_eq!(info.source, crate::xray::geo::GEO_SOURCE_NAME);

    // Verify files exist and have non-zero size
    let geoip = crate::xray::geo::active_dir(temp_data.path())
        .unwrap()
        .join("geoip.dat");
    let geosite = crate::xray::geo::active_dir(temp_data.path())
        .unwrap()
        .join("geosite.dat");
    assert!(geoip.is_file());
    assert!(geosite.is_file());
    assert!(std::fs::metadata(&geoip).unwrap().len() > 100_000);
    assert!(std::fs::metadata(&geosite).unwrap().len() > 1_000_000);
}

mod hwid_tests {
    use crate::{
        app::state::AppState,
        domain::errors::{BackendError, Result},
        platform::device_identity::{
            generate_hwid_from_seed, is_safe_header_value, is_valid_hwid, mask_hwid,
            read_device_model, read_machine_seed, read_os_version, DeviceIdentity,
            DeviceIdentityManager, DeviceIdentityStore, DEVICE_OS, HWID_PREFIX,
        },
        services::catalog_service,
        storage::secure::SecretStore,
        subscription::fetcher::{build_user_agent, fetch, fetch_with_metadata, SubscriptionClient},
    };
    use std::{collections::BTreeMap, net::SocketAddr, sync::Arc};
    use tokio::sync::Mutex;

    #[derive(Clone, Debug)]
    struct RecordedRequest {
        #[allow(dead_code)]
        method: String,
        #[allow(dead_code)]
        path: String,
        headers: BTreeMap<String, String>,
    }

    struct MockHttpServer {
        addr: SocketAddr,
        shutdown: Option<tokio::sync::oneshot::Sender<()>>,
        requests: Arc<Mutex<Vec<RecordedRequest>>>,
    }

    impl MockHttpServer {
        async fn start<F>(handler: F) -> Self
        where
            F: Fn(&str, &BTreeMap<String, String>) -> (u16, Vec<(&'static str, String)>, String)
                + Send
                + Sync
                + 'static,
        {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};

            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();
            let requests = Arc::new(Mutex::new(Vec::new()));
            let requests_clone = Arc::clone(&requests);
            let handler = Arc::new(handler);

            tokio::spawn(async move {
                loop {
                    tokio::select! {
                        _ = &mut shutdown_rx => break,
                        Ok((mut stream, _)) = listener.accept() => {
                            let requests = Arc::clone(&requests_clone);
                            let handler = Arc::clone(&handler);
                            tokio::spawn(async move {
                                let mut buf = vec![0u8; 8192];
                                let n = match stream.read(&mut buf).await {
                                    Ok(n) if n > 0 => n,
                                    _ => return,
                                };
                                let req_str = String::from_utf8_lossy(&buf[..n]);
                                let mut lines = req_str.lines();
                                let first_line = lines.next().unwrap_or("");
                                let mut parts = first_line.split_whitespace();
                                let method = parts.next().unwrap_or("GET").to_string();
                                let path = parts.next().unwrap_or("/").to_string();

                                let mut headers = BTreeMap::new();
                                for line in lines {
                                    if line.is_empty() || line == "\r" {
                                        break;
                                    }
                                    if let Some((k, v)) = line.split_once(':') {
                                        headers.insert(
                                            k.trim().to_ascii_lowercase(),
                                            v.trim().to_string(),
                                        );
                                    }
                                }

                                requests.lock().await.push(RecordedRequest {
                                    method,
                                    path: path.clone(),
                                    headers: headers.clone(),
                                });

                                let (status, resp_headers, body) = handler(&path, &headers);
                                let status_text = match status {
                                    200 => "OK",
                                    302 => "Found",
                                    400 => "Bad Request",
                                    403 => "Forbidden",
                                    404 => "Not Found",
                                    _ => "Unknown",
                                };

                                let mut resp = format!(
                                    "HTTP/1.1 {status} {status_text}\r\nContent-Length: {}\r\nConnection: close\r\n",
                                    body.len()
                                );
                                for (k, v) in resp_headers {
                                    resp.push_str(&format!("{k}: {v}\r\n"));
                                }
                                resp.push_str("\r\n");
                                resp.push_str(&body);

                                let _ = stream.write_all(resp.as_bytes()).await;
                                let _ = stream.flush().await;
                            });
                        }
                    }
                }
            });

            Self {
                addr,
                shutdown: Some(shutdown_tx),
                requests,
            }
        }

        fn url(&self, path: &str) -> String {
            format!("http://{}{path}", self.addr)
        }

        async fn recorded_requests(&self) -> Vec<RecordedRequest> {
            self.requests.lock().await.clone()
        }
    }

    impl Drop for MockHttpServer {
        fn drop(&mut self) {
            if let Some(shutdown) = self.shutdown.take() {
                let _ = shutdown.send(());
            }
        }
    }

    struct FailingSecretStore;
    impl SecretStore for FailingSecretStore {
        fn protect(&self, _input: &[u8]) -> Result<Vec<u8>> {
            Err(BackendError::new(
                "STORAGE_FAILED",
                "Simulated encryption failure",
            ))
        }
        fn unprotect(&self, _input: &[u8]) -> Result<Vec<u8>> {
            Err(BackendError::new(
                "STORAGE_FAILED",
                "Simulated decryption failure",
            ))
        }
    }

    const VALID_SUB_BASE64: &str =
        "dmxlc3M6Ly8wMDAwMDAwMC0wMDAwLTQwMDAtODAwMC0wMDAwMDAwMDAwMDFAMjAzLjAuMTEzLjEwOjQ0Mz9lbmNyeXB0aW9uPW5vbmUmc2VjdXJpdHk9bm9uZSZ0eXBlPXRjcCNQbGFpbi1WTEVTUw==";

    #[test]
    fn first_run_hwid_generation_and_valid_format() {
        let temp_dir = tempfile::tempdir().unwrap();
        let manager = DeviceIdentityManager::new(temp_dir.path().join("identity.dpapi"));
        let identity = manager.get_or_init().unwrap();

        assert!(is_valid_hwid(&identity.hwid));
        assert!(identity.hwid.starts_with(HWID_PREFIX));
        assert_eq!(identity.hwid.len(), 36);
        assert_eq!(identity.device_os, DEVICE_OS);
        assert!(is_safe_header_value(&identity.ver_os));

        // Format validation edge cases
        assert!(is_valid_hwid(&format!(
            "{HWID_PREFIX}0123456789abcdef0123456789abcdef"
        )));
        assert!(is_valid_hwid(&format!("{HWID_PREFIX}ABCDEF-1234=test")));
        assert!(!is_valid_hwid("linux-0123456789abcdef"));
        assert!(!is_valid_hwid(&format!("{HWID_PREFIX}123"))); // too short (< 10)
        assert!(!is_valid_hwid(&format!("{HWID_PREFIX}{}", "a".repeat(65)))); // too long (> 64)
        assert!(!is_valid_hwid(&format!("{HWID_PREFIX}invalid!char")));
    }

    #[test]
    fn same_hwid_across_repeated_calls_and_service_recreation() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("identity.dpapi");

        let manager1 = DeviceIdentityManager::new(path.clone());
        let id1 = manager1.get_or_init().unwrap();
        let id2 = manager1.get_or_init().unwrap();
        assert_eq!(id1.hwid, id2.hwid);

        // Recreate service from disk
        drop(manager1);
        let manager2 = DeviceIdentityManager::new(path);
        let id3 = manager2.get_or_init().unwrap();
        assert_eq!(id1.hwid, id3.hwid);
    }

    #[test]
    fn encrypted_persistence_and_no_raw_machine_guid_stored() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("identity.dpapi");
        let manager = DeviceIdentityManager::new(path.clone());
        let identity = manager.get_or_init().unwrap();

        let raw_bytes = std::fs::read(&path).unwrap();
        assert!(!raw_bytes.is_empty());
        assert!(raw_bytes.len() <= 4096);

        // Ciphertext must NOT match or contain plaintext HWID
        let raw_str = String::from_utf8_lossy(&raw_bytes);
        assert!(!raw_str.contains(&identity.hwid));

        // Must not contain raw MachineGuid
        if let Some(seed) = read_machine_seed() {
            assert!(!identity.hwid.contains(&seed));
            assert!(!raw_str.contains(&seed));
        }

        // Test synthetic machine seed
        let dummy_guid = "550e8400-e29b-41d4-a716-446655440000";
        let synthetic_hwid = generate_hwid_from_seed(dummy_guid);
        assert!(!synthetic_hwid.contains(dummy_guid));
        assert!(is_valid_hwid(&synthetic_hwid));

        // Test masked log output
        let masked = mask_hwid(&identity.hwid);
        assert!(masked.starts_with(HWID_PREFIX));
        assert!(masked.contains("..."));
        assert_ne!(masked, identity.hwid);
    }

    #[test]
    fn corrupted_identity_storage_and_decryption_failure() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("identity.dpapi");

        // 1. Oversized file (> 4096 bytes)
        std::fs::write(&path, vec![0x42; 5000]).unwrap();
        let store = DeviceIdentityStore::new(path.clone());
        let err = store.load().unwrap_err();
        assert_eq!(err.code, "HWID_UNAVAILABLE");

        // 2. Zero-byte file
        std::fs::write(&path, []).unwrap();
        let err = store.load().unwrap_err();
        assert_eq!(err.code, "HWID_UNAVAILABLE");

        // 3. Simulated decryption failure
        std::fs::write(&path, b"some-encrypted-data").unwrap();
        let failing_store =
            DeviceIdentityStore::with_store(path.clone(), Box::new(FailingSecretStore));
        let err = failing_store.load().unwrap_err();
        assert_eq!(err.code, "HWID_UNAVAILABLE");
    }

    #[test]
    fn actual_application_version_in_user_agent() {
        let ua = build_user_agent();
        let expected_prefix = format!("Lumen/{}", env!("CARGO_PKG_VERSION"));
        let expected_xray = format!("Xray/{}", crate::xray::bundle::VERSION);

        assert!(ua.starts_with(&expected_prefix));
        assert!(ua.contains(&expected_xray));
        assert!(ua.contains("Clash/1.19"));
    }

    #[test]
    fn os_version_and_device_model_safety() {
        let os_ver = read_os_version();
        assert!(is_safe_header_value(&os_ver));
        assert!(!os_ver.contains('\r') && !os_ver.contains('\n'));

        if let Some(model) = read_device_model() {
            assert!(is_safe_header_value(&model));
            assert!(!model.contains('\r') && !model.contains('\n'));
        }
    }

    #[tokio::test]
    async fn headers_sent_during_initial_import_and_refresh() {
        let server = MockHttpServer::start(|path, headers| {
            if path == "/sub" {
                assert_eq!(
                    headers.get("x-device-os").map(String::as_str),
                    Some(DEVICE_OS)
                );
                assert!(headers.contains_key("x-hwid"));
                assert!(headers.contains_key("x-ver-os"));
                assert!(headers.get("user-agent").unwrap().starts_with("Lumen/"));
                (
                    200,
                    vec![("content-type", "text/plain".into())],
                    VALID_SUB_BASE64.into(),
                )
            } else {
                (404, vec![], "Not Found".into())
            }
        })
        .await;

        let temp_dir = tempfile::tempdir().unwrap();
        let runtime = crate::runtime::RuntimePaths::new(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("runtime"),
        );
        let state = AppState::open(temp_dir.path().to_path_buf(), runtime).unwrap();

        // 1. Initial subscription import
        let import_url = server.url("/sub");
        let res =
            catalog_service::import(&state, import_url.clone(), Some("Test Sub".into()), None)
                .await
                .unwrap();
        assert_eq!(res.diagnostics.len(), 0);

        let requests = server.recorded_requests().await;
        assert_eq!(requests.len(), 1);
        let first_hwid = requests[0].headers.get("x-hwid").unwrap().clone();
        assert!(is_valid_hwid(&first_hwid));

        // 2. Refresh subscription
        let sub_id = state.repository.read().await.subscriptions[0].id.clone();
        let res_refresh =
            catalog_service::import(&state, import_url, Some("Test Sub".into()), Some(sub_id))
                .await
                .unwrap();
        assert_eq!(res_refresh.diagnostics.len(), 0);

        let requests_after = server.recorded_requests().await;
        assert_eq!(requests_after.len(), 2);
        let second_hwid = requests_after[1].headers.get("x-hwid").unwrap().clone();
        assert_eq!(first_hwid, second_hwid, "HWID must be identical on refresh");
    }

    #[tokio::test]
    async fn no_http_headers_for_inline_subscriptions() {
        let client = SubscriptionClient::ephemeral().unwrap();
        let inline = "vless://00000000-0000-4000-8000-000000000001@example.com:443?type=tcp#Inline";
        let fetched = fetch_with_metadata(inline, &client).await.unwrap();
        assert_eq!(fetched.body, inline);
    }

    #[tokio::test]
    async fn no_hwid_headers_on_unrelated_http_clients() {
        let server = MockHttpServer::start(|_path, headers| {
            assert!(!headers.contains_key("x-hwid"));
            assert!(!headers.contains_key("x-device-os"));
            assert!(!headers.contains_key("x-ver-os"));
            assert!(!headers.contains_key("x-device-model"));
            (200, vec![], "ip=1.2.3.4".into())
        })
        .await;

        let client = reqwest::Client::builder().build().unwrap();
        let resp = client.get(server.url("/trace")).send().await.unwrap();
        assert_eq!(resp.status(), 200);

        let requests = server.recorded_requests().await;
        assert_eq!(requests.len(), 1);
        assert!(!requests[0].headers.contains_key("x-hwid"));
    }

    #[tokio::test]
    async fn same_origin_redirect_preserves_hwid_headers() {
        let server = MockHttpServer::start(|path, headers| {
            if path == "/redirect-source" {
                assert!(headers.contains_key("x-hwid"));
                (
                    302,
                    vec![("location", "/redirect-target".into())],
                    "".into(),
                )
            } else if path == "/redirect-target" {
                assert!(headers.contains_key("x-hwid"));
                assert_eq!(
                    headers.get("x-device-os").map(String::as_str),
                    Some(DEVICE_OS)
                );
                (200, vec![], VALID_SUB_BASE64.into())
            } else {
                (404, vec![], "".into())
            }
        })
        .await;

        let client = SubscriptionClient::ephemeral().unwrap();
        let body = fetch(&server.url("/redirect-source"), &client)
            .await
            .unwrap();
        assert_eq!(body, VALID_SUB_BASE64);

        let requests = server.recorded_requests().await;
        assert_eq!(requests.len(), 2);
        assert!(requests[0].headers.contains_key("x-hwid"));
        assert!(requests[1].headers.contains_key("x-hwid"));
    }

    #[tokio::test]
    async fn no_hwid_leakage_through_cross_origin_redirects() {
        let server_b = MockHttpServer::start(|_path, headers| {
            // Target server B must NOT receive any HWID or device headers
            assert!(
                !headers.contains_key("x-hwid"),
                "x-hwid must not leak across origins"
            );
            assert!(
                !headers.contains_key("x-device-os"),
                "x-device-os must not leak across origins"
            );
            assert!(
                !headers.contains_key("x-ver-os"),
                "x-ver-os must not leak across origins"
            );
            assert!(
                !headers.contains_key("x-device-model"),
                "x-device-model must not leak across origins"
            );
            assert!(
                headers.contains_key("user-agent"),
                "user-agent must be preserved"
            );
            (200, vec![], VALID_SUB_BASE64.into())
        })
        .await;

        let target_url = server_b.url("/final-sub");
        let server_a = MockHttpServer::start(move |_path, headers| {
            // Origin A receives HWID
            assert!(headers.contains_key("x-hwid"));
            (302, vec![("location", target_url.clone())], "".into())
        })
        .await;

        let client = SubscriptionClient::ephemeral().unwrap();
        let body = fetch(&server_a.url("/entry"), &client).await.unwrap();
        assert_eq!(body, VALID_SUB_BASE64);

        let reqs_a = server_a.recorded_requests().await;
        let reqs_b = server_b.recorded_requests().await;
        assert_eq!(reqs_a.len(), 1);
        assert_eq!(reqs_b.len(), 1);
        assert!(reqs_a[0].headers.contains_key("x-hwid"));
        assert!(!reqs_b[0].headers.contains_key("x-hwid"));
    }

    #[tokio::test]
    async fn remnawave_error_headers_mapping() {
        // 1. x-hwid-not-supported: true (even with 404 or 403 or empty body)
        let s1 = MockHttpServer::start(|_path, _headers| {
            (
                404,
                vec![("x-hwid-not-supported", "true".into())],
                "".into(),
            )
        })
        .await;
        let client = SubscriptionClient::ephemeral().unwrap();
        let err1 = fetch(&s1.url("/sub"), &client).await.unwrap_err();
        assert_eq!(err1.code, "HWID_NOT_SUPPORTED");

        // 2. x-hwid-max-devices-reached: true
        let s2 = MockHttpServer::start(|_path, _headers| {
            (
                403,
                vec![("x-hwid-max-devices-reached", "true".into())],
                "".into(),
            )
        })
        .await;
        let err2 = fetch(&s2.url("/sub"), &client).await.unwrap_err();
        assert_eq!(err2.code, "HWID_DEVICE_LIMIT");

        // 3. x-hwid-limit: true
        let s3 = MockHttpServer::start(|_path, _headers| {
            (400, vec![("x-hwid-limit", "true".into())], "".into())
        })
        .await;
        let err3 = fetch(&s3.url("/sub"), &client).await.unwrap_err();
        assert_eq!(err3.code, "HWID_DEVICE_LIMIT");

        // 4. Case-insensitive header inspection
        let s4 = MockHttpServer::start(|_path, _headers| {
            (
                200,
                vec![("X-HWID-NOT-SUPPORTED", "TRUE".into())],
                "".into(),
            )
        })
        .await;
        let err4 = fetch(&s4.url("/sub"), &client).await.unwrap_err();
        assert_eq!(err4.code, "HWID_NOT_SUPPORTED");

        let s5 = MockHttpServer::start(|_path, _headers| {
            (200, vec![("X-Hwid-Limit", "1".into())], "".into())
        })
        .await;
        let err5 = fetch(&s5.url("/sub"), &client).await.unwrap_err();
        assert_eq!(err5.code, "HWID_DEVICE_LIMIT");
    }

    #[tokio::test]
    async fn ordinary_subscription_and_metadata_preservation() {
        let server = MockHttpServer::start(|_path, _headers| {
            (
                200,
                vec![
                    ("x-hwid-active", "true".into()),
                    (
                        "subscription-userinfo",
                        "upload=1000; download=2000; total=5000; expire=1800000000".into(),
                    ),
                    ("profile-title", "MySecureVPN".into()),
                ],
                VALID_SUB_BASE64.into(),
            )
        })
        .await;

        let client = SubscriptionClient::ephemeral().unwrap();
        let fetched = fetch_with_metadata(&server.url("/sub"), &client)
            .await
            .unwrap();

        assert_eq!(fetched.body, VALID_SUB_BASE64);
        assert_eq!(fetched.suggested_name, Some("MySecureVPN".into()));

        let user_info = fetched.user_info.unwrap();
        assert_eq!(user_info.upload, Some(1000));
        assert_eq!(user_info.download, Some(2000));
        assert_eq!(user_info.total, Some(5000));
        assert_eq!(user_info.expire, Some(1800000000));
    }

    #[tokio::test]
    async fn optional_device_model_header() {
        let server = MockHttpServer::start(|_path, headers| {
            if let Some(model) = headers.get("x-device-model") {
                assert_eq!(model, "Custom-Laptop-Pro");
            }
            (200, vec![], VALID_SUB_BASE64.into())
        })
        .await;

        // Identity with device model
        let id_with_model = DeviceIdentity {
            hwid: "win-0123456789abcdef0123456789abcdef".into(),
            device_os: "Windows".into(),
            ver_os: "10.0.22631".into(),
            device_model: Some("Custom-Laptop-Pro".into()),
        };
        let client_model = SubscriptionClient::with_identity(id_with_model);
        fetch(&server.url("/with-model"), &client_model)
            .await
            .unwrap();

        let reqs = server.recorded_requests().await;
        assert_eq!(reqs.len(), 1);
        assert_eq!(
            reqs[0].headers.get("x-device-model").map(String::as_str),
            Some("Custom-Laptop-Pro")
        );

        // Identity without device model
        let id_no_model = DeviceIdentity {
            hwid: "win-0123456789abcdef0123456789abcdef".into(),
            device_os: "Windows".into(),
            ver_os: "10.0.22631".into(),
            device_model: None,
        };
        let client_no_model = SubscriptionClient::with_identity(id_no_model);
        fetch(&server.url("/no-model"), &client_no_model)
            .await
            .unwrap();

        let reqs_after = server.recorded_requests().await;
        assert_eq!(reqs_after.len(), 2);
        assert!(!reqs_after[1].headers.contains_key("x-device-model"));
    }

    #[tokio::test]
    async fn profile_title_priority_over_generic_content_disposition() {
        let server = MockHttpServer::start(|_path, _headers| {
            (
                200,
                vec![
                    ("profile-title", "base64:0JPRgNC40LHQvdC40Lo=".into()),
                    (
                        "content-disposition",
                        "attachment; filename=\"config.yaml\"".into(),
                    ),
                ],
                VALID_SUB_BASE64.into(),
            )
        })
        .await;

        let client = SubscriptionClient::ephemeral().unwrap();
        let fetched = fetch_with_metadata(&server.url("/sub"), &client)
            .await
            .unwrap();

        assert_eq!(fetched.suggested_name, Some("Грибник".into()));
    }

    #[tokio::test]
    async fn profile_title_raw_utf8_bytes_support() {
        let server = MockHttpServer::start(|_path, _headers| {
            (
                200,
                vec![("profile-title", "Грибник".into())],
                VALID_SUB_BASE64.into(),
            )
        })
        .await;

        let client = SubscriptionClient::ephemeral().unwrap();
        let fetched = fetch_with_metadata(&server.url("/sub"), &client)
            .await
            .unwrap();

        assert_eq!(fetched.suggested_name, Some("Грибник".into()));
    }

    #[tokio::test]
    async fn subscription_body_profile_title_fallback() {
        let server = MockHttpServer::start(|_path, _headers| {
            (
                200,
                vec![],
                format!("# profile-title: RemnaTitle\n{VALID_SUB_BASE64}"),
            )
        })
        .await;

        let client = SubscriptionClient::ephemeral().unwrap();
        let fetched = fetch_with_metadata(&server.url("/sub"), &client)
            .await
            .unwrap();

        assert_eq!(fetched.suggested_name, Some("RemnaTitle".into()));
    }

    #[tokio::test]
    async fn subscription_import_name_url_fallback() {
        let server =
            MockHttpServer::start(|_path, _headers| (200, vec![], VALID_SUB_BASE64.into())).await;

        let temp_dir = tempfile::tempdir().unwrap();
        let runtime = crate::runtime::RuntimePaths::new(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("runtime"),
        );
        let state = AppState::open(temp_dir.path().to_path_buf(), runtime).unwrap();

        // 1. URL fragment name
        let url_with_fragment = format!(
            "{}#%D0%93%D1%80%D0%B8%D0%B1%D0%BD%D0%B8%D0%BA",
            server.url("/sub")
        );
        catalog_service::import(&state, url_with_fragment, None, None)
            .await
            .unwrap();
        assert_eq!(
            state.repository.read().await.subscriptions[0].name,
            "Грибник"
        );

        // 2. URL query name parameter
        let url_with_query = format!("{}?name=MyCustomService", server.url("/sub2"));
        catalog_service::import(&state, url_with_query, None, None)
            .await
            .unwrap();
        assert_eq!(
            state.repository.read().await.subscriptions[1].name,
            "MyCustomService"
        );
    }

    #[test]
    fn catalog_snapshot_infinite_days() {
        use crate::storage::repository::{Repository, Subscription};
        let mut repo = Repository::default();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Sub 1: expire == Some(0) (lifetime subscription standard)
        repo.subscriptions.push(Subscription {
            id: "sub-1".into(),
            name: "Infinite Days 0".into(),
            updated_at: now,
            expire: Some(0),
            total: Some(100_000_000_000),
            upload: Some(1000),
            download: Some(2000),
        });

        // Sub 2: expire == None but has user info (traffic)
        repo.subscriptions.push(Subscription {
            id: "sub-2".into(),
            name: "Infinite Days None".into(),
            updated_at: now,
            expire: None,
            total: Some(0), // infinite traffic too
            upload: Some(1000),
            download: Some(2000),
        });

        // Sub 3: expire in 20 years
        repo.subscriptions.push(Subscription {
            id: "sub-3".into(),
            name: "Far Future".into(),
            updated_at: now,
            expire: Some(now + 20 * 365 * 86_400),
            total: Some(500),
            upload: None,
            download: None,
        });

        // Sub 4: normal active subscription expiring in 5 days
        repo.subscriptions.push(Subscription {
            id: "sub-4".into(),
            name: "Normal 5 Days".into(),
            updated_at: now,
            expire: Some(now + 5 * 86_400 + 100),
            total: Some(500),
            upload: None,
            download: None,
        });

        let snap = crate::services::catalog_service::snapshot(&repo, &Default::default());
        let subs = snap["subscriptions"].as_array().unwrap();

        // Sub 1: infinite
        assert_eq!(subs[0]["daysRemaining"], "∞");
        assert_eq!(subs[0]["status"], "active");

        // Sub 2: infinite
        assert_eq!(subs[1]["daysRemaining"], "∞");
        assert_eq!(subs[1]["traffic"], "∞");
        assert_eq!(subs[1]["status"], "active");

        // Sub 3: far future -> infinite
        assert_eq!(subs[2]["daysRemaining"], "∞");
        assert_eq!(subs[2]["status"], "active");

        // Sub 4: normal
        assert_eq!(subs[3]["daysRemaining"], 5);
        assert_eq!(subs[3]["status"], "active");
    }
}

mod native;
mod reliability;
