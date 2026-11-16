//! The ping of an auto server: one probe core with a local port per member.

const ID: &str = "00000000-0000-4000-8000-000000000001";
const KEY: &str = "AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE";

fn auto_subscription() -> String {
    let member = |n: u8| {
        serde_json::json!({
            "tag": if n == 1 { "proxy".to_owned() } else { format!("proxy-{n}") },
            "protocol": "vless",
            "settings": {"vnext": [{"address": format!("203.0.113.{n}"), "port": 443,
                "users": [{"id": ID, "encryption": "none", "flow": "xtls-rprx-vision"}]}]},
            "streamSettings": {"network": "tcp", "security": "reality",
                "realitySettings": {"serverName": "www.apple.com", "publicKey": KEY,
                    "shortId": "abcd", "fingerprint": "firefox"}}
        })
    };
    serde_json::json!([{
        "remarks": "Авто",
        "outbounds": [member(1), member(2), member(3),
            {"tag": "direct", "protocol": "freedom"}, {"tag": "block", "protocol": "blackhole"}],
        "routing": {"balancers": [{"tag": "auto", "selector": ["proxy"],
            "strategy": {"type": "leastPing"}, "fallbackTag": "proxy"}]},
        "burstObservatory": {"subjectSelector": ["proxy"], "pingConfig": {
            "destination": "http://www.gstatic.com/generate_204", "interval": "30s",
            "sampling": 1, "timeout": "5s"}}
    }])
    .to_string()
}

/// The probe core of an auto server starts and opens a port for every member.
/// Its member ports used to take metrics + 1, the port of an HTTP inbound the
/// probe still had, so Xray exited and every auto server showed no ping.
#[cfg(any(windows, target_os = "macos"))]
#[tokio::test]
async fn auto_server_probe_core_starts_with_a_port_per_member() {
    let temp = tempfile::tempdir().unwrap();
    let runtime = crate::runtime::RuntimePaths::new(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("runtime"),
    );
    let state = crate::app::state::AppState::open(temp.path().to_path_buf(), runtime).unwrap();
    crate::services::catalog_service::import(&state, auto_subscription(), None, None)
        .await
        .unwrap();
    let id = state.repository.read().await.nodes[0].id.clone();
    for _ in 0..3 {
        let core = crate::services::latency_service::isolated_core(&state, &id, true)
            .await
            .expect("the probe core of an auto server starts");
        assert_eq!(core.members.len(), 3);
        let mut ports: Vec<u16> = core.members.iter().map(|(_, port)| *port).collect();
        ports.push(core.proxy_port);
        ports.sort_unstable();
        ports.dedup();
        assert_eq!(ports.len(), 4);
        core.stop().await;
    }
}

#[cfg(any(windows, target_os = "macos", target_os = "android"))]
#[test]
fn probe_ports_never_take_the_core_ports() {
    let (proxy, metrics) = crate::engine::xray::ports().unwrap();
    let taken = [proxy, metrics, metrics + 1, metrics + 2];
    let mut ports = crate::services::latency_service::probe_ports(16, &taken).unwrap();
    assert!(ports.iter().all(|port| !taken.contains(port)));
    ports.sort_unstable();
    ports.dedup();
    assert_eq!(ports.len(), 16);
}
