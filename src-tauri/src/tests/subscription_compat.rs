//! A server is skipped only when Xray cannot run it or when it asks to turn
//! certificate checks off. Settings Lumen does not model must never hide a
//! server: they travel to the core in the provider's own outbound.
use super::*;
use serde_json::{json, Value};

const ID: &str = "00000000-0000-4000-8000-000000000001";
const KEY: &str = "AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE";

fn reality_link(extra_query: &str) -> String {
    format!("vless://{ID}@203.0.113.10:443?type=tcp&security=reality&sni=www.apple.com&fp=firefox&pbk={KEY}&sid=abcd&flow=xtls-rprx-vision&encryption=none{extra_query}#Reality")
}

/// The CDN host of a whitelist subscription: XHTTP over TLS with `extra`.
fn xhttp_extra_link() -> String {
    let extra = json!({
        "uplinkHTTPMethod": "GET",
        "uplinkDataPlacement": "header",
        "scMaxEachPostBytes": "8000-10000",
        "scMinPostsIntervalMs": "10-30"
    });
    format!(
        "vless://{ID}@cdn.example.com:443?encryption=none&type=xhttp&path=%2Fstatic%2Fabc&host=cdn.example.com&mode=packet-up&extra={}&security=tls&sni=cdn.example.com&fp=firefox&alpn=h2%2Chttp%2F1.1#CDN",
        percent_encoding::utf8_percent_encode(&extra.to_string(), percent_encoding::NON_ALPHANUMERIC)
    )
}

fn reality_outbound() -> Value {
    json!({
        "tag": "proxy",
        "protocol": "vless",
        "settings": {"vnext": [{"address": "203.0.113.10", "port": 443,
            "users": [{"id": ID, "encryption": "none", "flow": "xtls-rprx-vision"}]}]},
        "streamSettings": {"network": "tcp", "tcpSettings": {}, "security": "reality",
            "realitySettings": {"serverName": "www.apple.com", "publicKey": KEY,
                "shortId": "abcd", "fingerprint": "firefox"}}
    })
}

/// An auto server whose first member is a loopback placeholder and whose real
/// members all use settings Lumen does not model.
fn provider_group() -> Value {
    json!({
        "remarks": "Белые списки | DE",
        "outbounds": [
            {"tag": "proxy", "protocol": "vless", "settings": {"vnext": [{"address": "127.0.0.1", "port": 1234,
                "users": [{"id": ID, "encryption": "none"}]}]}},
            {"tag": "proxy-2", "protocol": "trojan",
                "settings": {"servers": [{"address": "203.0.113.20", "port": 443, "password": "secret"}]},
                "streamSettings": {"network": "tcp", "security": "tls",
                    "tlsSettings": {"serverName": "example.com", "fingerprint": "chrome"},
                    "finalmask": {"tcp": [{"type": "fragment", "settings": {"packets": "tlshello", "lengths": ["1-1"], "delays": ["0"]}}]}}},
            {"tag": "proxy-3", "protocol": "vless",
                "settings": {"vnext": [{"address": "203.0.113.30", "port": 443, "users": [{"id": ID, "encryption": "none"}]}]},
                "streamSettings": {"network": "ws", "security": "tls",
                    "tlsSettings": {"serverName": "ws.example.com"},
                    "wsSettings": {"host": "ws.example.com", "path": "/ws", "headers": {}}}},
            {"tag": "proxy-4", "protocol": "vless",
                "settings": {"vnext": [{"address": "203.0.113.40", "port": 443, "users": [{"id": ID, "encryption": "none"}]}]},
                "streamSettings": {"network": "grpc", "security": "tls",
                    "tlsSettings": {"serverName": "grpc.example.com"},
                    "grpcSettings": {"serviceName": "grpc", "authority": "grpc.example.com"},
                    "sockopt": {"tcpKeepAliveIdle": 100, "tcpNoDelay": true}}},
            {"tag": "direct", "protocol": "freedom"}
        ],
        "routing": {"balancers": [{"tag": "auto", "selector": ["proxy"], "strategy": {"type": "leastPing"}}]},
        "burstObservatory": {"subjectSelector": ["proxy"], "pingConfig": {"destination": "https://www.gstatic.com/generate_204", "interval": "30s"}}
    })
}

#[test]
fn xhttp_extra_reaches_the_core() {
    let node = uri::parse(&xhttp_extra_link(), "sub").unwrap();
    let outbound = config::outbound(&node, "proxy").unwrap();
    assert_eq!(outbound["tag"], "proxy");
    let xhttp = &outbound["streamSettings"]["xhttpSettings"];
    assert_eq!(xhttp["mode"], "packet-up");
    assert_eq!(xhttp["extra"]["uplinkHTTPMethod"], "GET");
    assert_eq!(xhttp["extra"]["uplinkDataPlacement"], "header");
    assert_eq!(
        outbound["streamSettings"]["tlsSettings"]["alpn"],
        json!(["h2", "http/1.1"])
    );

    let list = format!("{}\n{}", reality_link(""), xhttp_extra_link());
    use base64::Engine;
    let parsed = parser::parse(
        &base64::engine::general_purpose::STANDARD.encode(list),
        "sub",
    )
    .unwrap();
    assert_eq!(parsed.nodes.len(), 2);
    assert!(parsed.diagnostics.is_empty());
}

#[test]
fn unknown_link_keys_never_hide_a_server_or_change_its_id() {
    let plain = uri::parse(&reality_link(""), "sub").unwrap();
    assert!(plain.outbound.is_none());
    let hinted = uri::parse(&reality_link("&futureHint=1&ed=2048&udp=true"), "sub").unwrap();
    assert_eq!(hinted.id, plain.id);
    assert!(hinted.outbound.is_none());

    let node = uri::parse(&reality_link("&spx=%2Fhome&pqv=verify-key"), "sub").unwrap();
    assert_eq!(node.security.spider_x, "/home");
    let reality = &config::outbound(&node, "proxy").unwrap()["streamSettings"]["realitySettings"];
    assert_eq!(reality["spiderX"], "/home");
    assert_eq!(reality["mldsa65Verify"], "verify-key");

    // Lumen's defaults are the strict ones; asking to skip certificate checks still skips.
    assert!(uri::parse(&reality_link("&allowInsecure=1"), "sub").is_err());
    assert!(uri::parse("trojan://password@example.com:443?insecure=true", "sub").is_err());
}

#[test]
fn link_settings_xray_needs_are_kept() {
    let encrypted = format!(
        "vless://{ID}@203.0.113.10:443?type=tcp&security=none&encryption=mlkem768x25519plus.native.0rtt.client-key#PQ"
    );
    let outbound = config::outbound(&uri::parse(&encrypted, "sub").unwrap(), "proxy").unwrap();
    assert_eq!(
        outbound["settings"]["vnext"][0]["users"][0]["encryption"],
        "mlkem768x25519plus.native.0rtt.client-key"
    );

    let hysteria = "hysteria2://password@example.com:443?sni=example.com&obfs=salamander&obfs-password=mask#Hy2";
    let outbound = config::outbound(&uri::parse(hysteria, "sub").unwrap(), "proxy").unwrap();
    assert_eq!(
        outbound["streamSettings"]["finalmask"],
        json!({"udp": [{"type": "salamander", "settings": {"password": "mask"}}]})
    );
    assert!(uri::parse(
        "hysteria2://password@example.com:443?obfs=unknown-obfs&obfs-password=x",
        "sub"
    )
    .is_err());

    let header = format!("vless://{ID}@203.0.113.10:80?type=tcp&headerType=http&host=a.example%2Cb.example&path=%2Fx#Header");
    let outbound = config::outbound(&uri::parse(&header, "sub").unwrap(), "proxy").unwrap();
    assert_eq!(
        outbound["streamSettings"]["rawSettings"]["header"],
        json!({"type": "http", "request": {"path": ["/x"], "headers": {"Host": ["a.example", "b.example"]}}})
    );

    let grpc = format!("vless://{ID}@203.0.113.10:443?type=grpc&security=tls&sni=example.com&serviceName=svc&authority=auth.example#gRPC");
    let outbound = config::outbound(&uri::parse(&grpc, "sub").unwrap(), "proxy").unwrap();
    assert_eq!(
        outbound["streamSettings"]["grpcSettings"]["authority"],
        "auth.example"
    );
}

#[test]
fn unknown_outbound_keys_at_any_level_keep_the_server() {
    let modeled = xray_json_node(&reality_outbound());
    assert!(modeled.outbound.is_none(), "a modeled server keeps its id");
    let paths: &[&[&str]] = &[
        &[],
        &["settings"],
        &["settings", "vnext", "0"],
        &["settings", "vnext", "0", "users", "0"],
        &["streamSettings"],
        &["streamSettings", "realitySettings"],
        &["streamSettings", "tcpSettings"],
    ];
    for path in paths {
        let mut outbound = reality_outbound();
        let mut target = &mut outbound;
        for key in *path {
            target = match key.parse::<usize>() {
                Ok(index) => &mut target[index],
                Err(_) => &mut target[*key],
            };
        }
        target["futureOption"] = json!(7);
        let node = xray_json_node(&outbound);
        assert_eq!(node.address, "203.0.113.10", "{path:?}");
        let built = config::outbound(&node, "proxy").unwrap();
        let mut kept = &built;
        for key in *path {
            kept = match key.parse::<usize>() {
                Ok(index) => &kept[index],
                Err(_) => &kept[*key],
            };
        }
        assert_eq!(kept["futureOption"], 7, "{path:?}");
        assert!(built.get("remarks").is_none());
    }
}

#[test]
fn provider_groups_and_settings_stay_listed() {
    let parsed = parser::parse(&json!([provider_group()]).to_string(), "sub").unwrap();
    assert!(parsed.diagnostics.is_empty());
    assert_eq!(parsed.nodes.len(), 1);
    let group = &parsed.nodes[0];
    assert_eq!(group.name, "Белые списки | DE");
    let balancer = group.balancer.as_ref().unwrap();
    assert_eq!(balancer.outbounds.len(), 3, "the placeholder is dropped");
    assert!(balancer.outbounds[0]["streamSettings"]
        .get("finalmask")
        .is_some());

    let mut single = provider_group()["outbounds"][2].clone();
    single["remarks"] = json!("WS");
    let node = xray_json_node(&single);
    assert_eq!(node.transport.host, "ws.example.com");
    let built = config::outbound(&node, "proxy").unwrap();
    assert_eq!(
        built["streamSettings"]["wsSettings"]["host"],
        "ws.example.com"
    );

    let inline = json!({"protocol": "vless", "settings": {"address": "203.0.113.50", "port": 443,
        "id": ID, "encryption": "none"}});
    assert_eq!(xray_json_node(&inline).address, "203.0.113.50");
}

#[test]
fn servers_xray_cannot_run_alone_stay_out() {
    let mut chained = reality_outbound();
    chained["streamSettings"]["sockopt"] = json!({"dialerProxy": "other"});
    assert!(parser::parse(&chained.to_string(), "sub").is_err());
    let mut insecure = provider_group()["outbounds"][1].clone();
    insecure["streamSettings"]["tlsSettings"]["allowInsecure"] = json!(true);
    assert!(parser::parse(&insecure.to_string(), "sub").is_err());
}

#[test]
fn clash_tuning_keys_do_not_hide_a_proxy() {
    let yaml = format!(
        r#"proxies:
  - {{name: tuned, type: vless, server: 203.0.113.10, port: 443, uuid: {ID}, tls: true, servername: ws.example.com, network: ws, tfo: true, ip-version: ipv4, smux: {{enabled: false}}, ws-opts: {{path: /ws, max-early-data: 2048, headers: {{Host: ws.example.com, User-Agent: x}}}}}}
  - {{name: upgrade, type: vless, server: 203.0.113.11, port: 443, uuid: {ID}, tls: true, network: ws, ws-opts: {{path: /up, v2ray-http-upgrade: true}}}}
  - {{name: hy2, type: hysteria2, server: 203.0.113.12, port: 443, password: pw, obfs: salamander, obfs-password: mask, sni: example.com}}
  - {{name: plugin, type: ss, server: 203.0.113.13, port: 8388, cipher: aes-256-gcm, password: pw, plugin: obfs}}
"#
    );
    let parsed = parser::parse(&yaml, "sub").unwrap();
    let names: Vec<_> = parsed.nodes.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(names, ["tuned", "upgrade", "hy2"]);
    assert_eq!(
        parsed.diagnostics.len(),
        1,
        "a plugin needs a runtime Xray lacks"
    );
    let tuned = config::outbound(&parsed.nodes[0], "proxy").unwrap();
    let ws = &tuned["streamSettings"]["wsSettings"];
    assert_eq!(ws["path"], "/ws");
    assert_eq!(
        ws["headers"],
        json!({"Host": "ws.example.com", "User-Agent": "x"})
    );
    assert!(parsed.nodes[1].outbound.is_none());
    assert_eq!(parsed.nodes[1].transport.network, "httpupgrade");
    let hy2 = config::outbound(&parsed.nodes[2], "proxy").unwrap();
    assert_eq!(
        hy2["streamSettings"]["finalmask"]["udp"][0]["type"],
        "salamander"
    );
}

#[test]
fn wireguard_with_newer_options_stays_listed() {
    let outbound = json!({"protocol": "wireguard", "settings": {
        "secretKey": "AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE=", "address": ["10.0.0.2/32"],
        "workers": 2,
        "peers": [{"endpoint": "example.com:51820", "publicKey": "AgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgI=",
            "allowedIPs": ["0.0.0.0/0"], "futurePeerOption": true}]}});
    let node = xray_json_node(&outbound);
    let built = config::outbound(&node, "proxy").unwrap();
    assert_eq!(built["settings"]["workers"], 2);
}

/// The servers above, as the bundled core runs them.
#[tokio::test]
async fn bundled_core_runs_provider_outbounds() {
    let directory = tempfile::tempdir().unwrap();
    let runtime = crate::runtime::RuntimePaths::new(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("runtime"),
    );
    let core = bundle::prepare(&runtime, directory.path()).unwrap();
    let mut nodes = vec![
        uri::parse(&xhttp_extra_link(), "sub").unwrap(),
        uri::parse("hysteria2://password@example.com:443?sni=example.com&obfs=salamander&obfs-password=mask", "sub").unwrap(),
        uri::parse(&format!("vless://{ID}@203.0.113.10:80?type=tcp&headerType=http&host=a.example&path=%2Fx"), "sub").unwrap(),
    ];
    nodes.extend(
        parser::parse(&json!([provider_group()]).to_string(), "group")
            .unwrap()
            .nodes,
    );
    let mut single = provider_group()["outbounds"][3].clone();
    single["remarks"] = json!("gRPC");
    nodes.push(xray_json_node(&single));
    let repo = Repository {
        nodes,
        ..Default::default()
    };
    for node in &repo.nodes {
        for tun in [false, true] {
            let config = config::build(&repo, &node.id, 10808, 10809, tun).unwrap();
            let bytes = serde_json::to_vec(&config).unwrap();
            if let Err(error) = crate::engine::xray::validate(&core, &bytes).await {
                panic!("{} (tun {tun}): {}", node.name, error.code);
            }
        }
    }
}

fn xray_json_node(outbound: &Value) -> crate::domain::node::VpnNode {
    let mut parsed = parser::parse(&outbound.to_string(), "sub").unwrap();
    assert!(parsed.diagnostics.is_empty());
    parsed.nodes.remove(0)
}

/// What Lumen still cannot run is counted on the subscription card, so a
/// server never disappears without a trace again.
#[tokio::test]
async fn skipped_servers_are_counted_on_the_card() {
    let temp = tempfile::tempdir().unwrap();
    let runtime = crate::runtime::RuntimePaths::new(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("runtime"),
    );
    let state = crate::app::state::AppState::open(temp.path().to_path_buf(), runtime).unwrap();
    let list = format!(
        "{}\n{}\n{}",
        reality_link(""),
        xhttp_extra_link(),
        reality_link("&allowInsecure=1")
    );
    let imported = crate::services::catalog_service::import(&state, list, Some("Sub".into()), None)
        .await
        .unwrap();
    assert_eq!(imported.diagnostics.len(), 1);
    assert_eq!(imported.catalog["subscriptions"][0]["skipped"], 1);
}
