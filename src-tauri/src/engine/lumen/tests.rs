use super::*;
use crate::engine::{backend, BackendSession, Session};
use crate::storage::repository::{RepositoryStore, Rule, RuleMatcher};
use std::time::Duration;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

fn repository(port: u16, protocol: &str) -> Repository {
    let node =
        crate::subscription::uri::parse(&format!("{protocol}://127.0.0.1:{port}#Native"), "native")
            .unwrap();
    let mut repo = Repository {
        nodes: vec![node.clone()],
        selected_server_id: Some(node.id),
        ..Default::default()
    };
    repo.settings.core_engine = CoreEngine::Lumen;
    repo.settings.network_mode = "proxy".into();
    repo.settings.dns_server = "system".into();
    repo.settings.bypass_lan = false;
    repo
}
fn rule(name: &str, target: &str) -> Rule {
    Rule {
        id: uuid::Uuid::new_v4().to_string(),
        name: name.into(),
        matcher: RuleMatcher::Domain,
        server_id: target.into(),
        enabled: true,
        preset_id: None,
        is_domain: true,
    }
}
async fn echo() -> (std::net::SocketAddr, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let mut flows = tokio::task::JoinSet::new();
        loop {
            tokio::select! {
                incoming = listener.accept() => {
                    let Ok((mut stream,_)) = incoming else { break; };
                    flows.spawn(async move { let (mut r,mut w)=stream.split(); let _=tokio::io::copy(&mut r,&mut w).await; });
                }
                _=flows.join_next(),if !flows.is_empty()=>{},
            }
        }
    });
    (address, task)
}
async fn request(port: u16, destination: std::net::SocketAddr) -> TcpStream {
    let mut stream = TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port))
        .await
        .unwrap();
    outbound::socks_connect(
        &mut stream,
        &outbound::Destination::new(destination.ip().to_string(), destination.port()).unwrap(),
        "",
        "",
    )
    .await
    .unwrap();
    stream.write_all(b"native-payload").await.unwrap();
    let mut bytes = [0; 14];
    tokio::time::timeout(Duration::from_secs(3), stream.read_exact(&mut bytes))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(&bytes, b"native-payload");
    stream
}

#[test]
fn engine_migration_capabilities_and_encrypted_restart() {
    let mut legacy = serde_json::to_value(Repository::default()).unwrap();
    legacy["settings"]
        .as_object_mut()
        .unwrap()
        .remove("coreEngine");
    let mut repo: Repository = serde_json::from_value(legacy).unwrap();
    assert_eq!(repo.settings.core_engine, CoreEngine::Xray);
    let directory = tempfile::tempdir().unwrap();
    let store = RepositoryStore::new(directory.path()).unwrap();
    repo.settings.core_engine = CoreEngine::Lumen;
    store.save(&repo).unwrap();
    assert_eq!(
        store.load().unwrap().settings.core_engine,
        CoreEngine::Lumen
    );
    assert!(backend(CoreEngine::Xray).capabilities().reality);
    assert!(!backend(CoreEngine::Lumen).capabilities().tun);
    assert!(!backend(CoreEngine::Lumen).capabilities().udp);
    assert_eq!(
        backend(CoreEngine::Lumen).capabilities().protocols,
        vec!["socks", "http"]
    );
    let mut value = serde_json::to_value(repo).unwrap();
    value["settings"]["coreEngine"] = serde_json::json!("unknown");
    assert!(serde_json::from_value::<Repository>(value).is_err());
}
#[test]
fn unsupported_profiles_are_rejected_without_fallback() {
    let mut repo = repository(1080, "socks5");
    let selected = repo.nodes[0].id.clone();
    assert!(LumenBackend.validate(&repo, &selected).is_ok());
    repo.settings.network_mode = "tun".into();
    assert_eq!(
        LumenBackend.validate(&repo, &selected).unwrap_err().code,
        "LUMEN_UNSUPPORTED_CONFIGURATION"
    );
    repo.settings.network_mode = "proxy".into();
    repo.nodes[0].protocol = Protocol::Vless;
    repo.nodes[0].credential = "00000000-0000-4000-8000-000000000001".into();
    assert_eq!(
        LumenBackend.validate(&repo, &selected).unwrap_err().code,
        "LUMEN_UNSUPPORTED_CONFIGURATION"
    );
    assert!(backend(CoreEngine::Xray).validate(&repo, &selected).is_ok());
}
#[test]
fn proxy_subscription_roundtrip_and_credential_boundary() {
    for input in [
        "socks5://name:pass%3Aword@127.0.0.1:1080",
        "http-proxy://name:pass%3Aword@127.0.0.1:8080",
        "socks5://127.0.0.1:1080",
    ] {
        let node = crate::subscription::uri::parse(input, "native").unwrap();
        let json = crate::xray::config::outbound(&node, "proxy").unwrap();
        let parsed = crate::subscription::parser::parse(&json.to_string(), "native")
            .unwrap()
            .nodes
            .remove(0);
        assert_eq!(node.credential, parsed.credential);
        assert_eq!(node.method, parsed.method);
        assert_eq!(node.protocol, parsed.protocol);
        let repo = Repository {
            nodes: vec![node],
            ..Default::default()
        };
        let dto =
            crate::services::catalog_service::snapshot(&repo, &Default::default()).to_string();
        assert!(!dto.contains("pass:word"));
        assert!(!dto.contains("credential"));
        assert!(!dto.contains("method"));
    }
}

#[tokio::test]
async fn native_http_forwarding_strips_proxy_credentials() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let origin = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let head = outbound::read_headers(&mut stream).await.unwrap();
        assert!(head.starts_with("GET /proof?q=1 HTTP/1.1"));
        assert!(!head.to_lowercase().contains("proxy-authorization"));
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\nnative")
            .await
            .unwrap();
    });
    let mut repo = repository(9, "socks5");
    repo.routing_mode = "rules".into();
    let session = LumenBackend::start_native(&repo, &repo.nodes[0].id)
        .await
        .unwrap();
    let client = reqwest::Client::builder()
        .no_proxy()
        .proxy(reqwest::Proxy::all(format!("http://127.0.0.1:{}", session.port)).unwrap())
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let body = client
        .get(format!("http://{address}/proof?q=1"))
        .header("Proxy-Authorization", "fixture-secret")
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert_eq!(body, "native");
    session.shutdown().await;
    origin.await.unwrap();
}
#[tokio::test]
async fn http_forwarding_cannot_pipeline_a_second_unrouted_request() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let origin = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let head = outbound::read_headers(&mut stream).await.unwrap();
        assert!(head.starts_with("GET /first HTTP/1.1"));
        let mut next = [0; 1];
        assert_eq!(stream.read(&mut next).await.unwrap(), 0);
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK")
            .await
            .unwrap();
    });
    let mut repo = repository(9, "socks5");
    repo.routing_mode = "rules".into();
    let session = LumenBackend::start_native(&repo, &repo.nodes[0].id)
        .await
        .unwrap();
    let mut client = TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, session.port))
        .await
        .unwrap();
    client.write_all(format!(
        "GET http://{address}/first HTTP/1.1\r\nHost: {address}\r\n\r\nGET http://{address}/second HTTP/1.1\r\nHost: {address}\r\n\r\n"
    ).as_bytes()).await.unwrap();
    let response = tokio::time::timeout(Duration::from_secs(3), async {
        let mut bytes = Vec::new();
        client.read_to_end(&mut bytes).await.unwrap();
        bytes
    })
    .await
    .unwrap();
    assert!(response.starts_with(b"HTTP/1.1 200 OK"));
    origin.await.unwrap();
    session.shutdown().await;
}
#[test]
fn native_route_priority_suffix_boundaries_and_disabled_rules() {
    let mut repo = repository(1080, "socks5");
    repo.routing_mode = "rules".into();
    repo.rules = vec![
        rule("example.com", "block"),
        rule("child.example.com", "direct"),
        rule("10.0.0.0/8", "block"),
    ];
    let table = RoutingTable::compile(&repo, &repo.nodes[0].id).unwrap();
    let d = |name: &str| outbound::Destination::new(name.into(), 443).unwrap();
    assert_eq!(table.target(&d("child.example.com"), &[]), 1);
    assert_eq!(table.target(&d("notexample.com"), &[]), 0);
    assert_eq!(table.target(&d("10.2.3.4"), &[]), 1);
    assert_eq!(
        table.target(&d("other.test"), &["10.2.3.4".parse().unwrap()]),
        1
    );
    repo.rules[0].enabled = false;
    let table = RoutingTable::compile(&repo, &repo.nodes[0].id).unwrap();
    assert_eq!(table.target(&d("child.example.com"), &[]), 0);
    repo.rules[1].matcher = RuleMatcher::Application;
    assert!(RoutingTable::compile(&repo, &repo.nodes[0].id).is_err());
    repo.routing_mode = "unexpected".into();
    assert_eq!(
        RoutingTable::compile(&repo, &repo.nodes[0].id)
            .err()
            .unwrap()
            .code,
        "LUMEN_UNSUPPORTED_CONFIGURATION"
    );
}

#[tokio::test]
async fn global_lan_hostname_bypasses_unreachable_proxy() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let origin = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut payload = [0; 4];
        stream.read_exact(&mut payload).await.unwrap();
        stream.write_all(&payload).await.unwrap();
    });
    let mut repo = repository(9, "socks5");
    repo.routing_mode = "global".into();
    repo.settings.bypass_lan = true;
    let session = LumenBackend::start_native(&repo, &repo.nodes[0].id)
        .await
        .unwrap();
    let mut client = TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, session.port))
        .await
        .unwrap();
    outbound::socks_connect(
        &mut client,
        &outbound::Destination::new("localhost".into(), port).unwrap(),
        "",
        "",
    )
    .await
    .unwrap();
    client.write_all(b"LAN!").await.unwrap();
    let mut payload = [0; 4];
    client.read_exact(&mut payload).await.unwrap();
    assert_eq!(&payload, b"LAN!");
    origin.await.unwrap();
    session.shutdown().await;
}

#[tokio::test]
async fn native_direct_block_live_update_udp_rejection_stats_and_cleanup() {
    let (destination, origin) = echo().await;
    let mut repo = repository(9, "socks5");
    repo.routing_mode = "rules".into();
    repo.rules = vec![rule("127.0.0.1", "direct")];
    let session = LumenBackend::start_native(&repo, &repo.nodes[0].id)
        .await
        .unwrap();
    let mut active = request(session.port, destination).await;
    let (up, down) = session.counters.read();
    assert!(up >= 14 && down >= 14);
    repo.rules[0].server_id = "block".into();
    *session.routing.write().await =
        Arc::new(RoutingTable::compile(&repo, &repo.nodes[0].id).unwrap());
    let mut blocked = TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, session.port))
        .await
        .unwrap();
    assert!(outbound::socks_connect(
        &mut blocked,
        &outbound::Destination::new(destination.ip().to_string(), destination.port()).unwrap(),
        "",
        ""
    )
    .await
    .is_err());
    let mut udp = TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, session.port))
        .await
        .unwrap();
    udp.write_all(&[5, 1, 0, 5, 3, 0, 1, 0, 0, 0, 0, 0, 0])
        .await
        .unwrap();
    let mut response = [0; 12];
    udp.read_exact(&mut response).await.unwrap();
    assert_eq!(response[3], 7);
    let port = session.port;
    session.shutdown().await;
    assert!(TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port))
        .await
        .is_err());
    let mut byte = [0];
    let result = tokio::time::timeout(Duration::from_secs(2), active.read(&mut byte))
        .await
        .unwrap();
    assert!(matches!(result, Ok(0) | Err(_)));
    origin.abort();
}

#[cfg(windows)]
#[tokio::test]
async fn native_socks_http_interoperate_with_xray_and_switch_engines() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = crate::runtime::RuntimePaths::new(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("runtime"),
    );
    let state = AppState::open(dir.path().into(), runtime).unwrap();
    let (destination, origin) = echo().await;
    for protocol in ["socks", "http"] {
        let (port, metrics) = crate::engine::xray::ports().unwrap();
        let config = serde_json::json!({"log":{"loglevel":"none"},"inbounds":[{"listen":"127.0.0.1","port":port,"protocol":protocol,"settings":{"auth":"password","accounts":[{"user":"fixture","pass":"test-password"}]}}],"outbounds":[{"protocol":"freedom"}]});
        let mut upstream = crate::engine::xray::XraySession::default();
        crate::engine::xray::start(
            &state.core.read().await,
            &mut upstream,
            &serde_json::to_vec(&config).unwrap(),
            port,
            metrics,
        )
        .await
        .unwrap();
        let scheme = if protocol == "socks" {
            "socks5"
        } else {
            "http-proxy"
        };
        let mut repo = repository(port, scheme);
        repo.nodes[0].method = "fixture".into();
        repo.nodes[0].credential = "test-password".into();
        let selected = repo.nodes[0].id.clone();
        let mut session = Session::default();
        // Both directions, through the same manager and same native socket proof.
        for engine in [
            CoreEngine::Xray,
            CoreEngine::Lumen,
            CoreEngine::Xray,
            CoreEngine::Lumen,
        ] {
            repo.settings.core_engine = engine;
            crate::engine::connect(&state, &repo, &selected, &mut session)
                .await
                .unwrap();
            assert_eq!(session.engine(), engine);
            assert!(session.started().is_some());
            let flow = request(session.proxy_port(), destination).await;
            drop(flow);
            // Persisting the other choice cannot retag a live backend.
            let mut saved = repo.clone();
            saved.settings.core_engine = if engine == CoreEngine::Xray {
                CoreEngine::Lumen
            } else {
                CoreEngine::Xray
            };
            state.store.save(&saved).unwrap();
            assert_eq!(session.engine(), engine);
            assert!(!session.failed());
            let previous = session.rollback_repository(&saved);
            assert_eq!(previous.settings.core_engine, engine);
            assert_eq!(previous.settings.network_mode, "proxy");
            if engine == CoreEngine::Lumen {
                let mut http =
                    TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, session.http_port()))
                        .await
                        .unwrap();
                http.write_all(
                    format!("CONNECT {destination} HTTP/1.1\r\nHost: {destination}\r\n\r\n")
                        .as_bytes(),
                )
                .await
                .unwrap();
                assert!(outbound::read_headers(&mut http)
                    .await
                    .unwrap()
                    .starts_with("HTTP/1.1 200"));
                http.write_all(b"connect-proof").await.unwrap();
                let mut bytes = [0; 13];
                http.read_exact(&mut bytes).await.unwrap();
                assert_eq!(&bytes, b"connect-proof");
            }
            let old_port = session.proxy_port();
            crate::services::vpn_service::stop(&mut session)
                .await
                .unwrap();
            assert!(session.started().is_none());
            assert!(
                TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, old_port))
                    .await
                    .is_err()
            );
        }
        // Failed validation does not start an Xray fallback.
        repo.nodes[0].security.kind = "tls".into();
        assert_eq!(
            crate::engine::connect(&state, &repo, &selected, &mut session)
                .await
                .unwrap_err()
                .code,
            "LUMEN_UNSUPPORTED_CONFIGURATION"
        );
        assert!(matches!(session.backend, BackendSession::Lumen(None)));
        crate::engine::xray::stop(&mut upstream).await.unwrap();
    }
    origin.abort();
}

#[cfg(windows)]
#[tokio::test]
async fn failed_native_switch_can_restore_previous_xray_vless_connection() {
    let directory = tempfile::tempdir().unwrap();
    let runtime = crate::runtime::RuntimePaths::new(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("runtime"),
    );
    let state = AppState::open(directory.path().into(), runtime).unwrap();
    let (destination, origin) = echo().await;
    let (port, metrics) = crate::engine::xray::ports().unwrap();
    let user = "00000000-0000-4000-8000-000000000001";
    let config = serde_json::json!({
        "log":{"loglevel":"none"},
        "inbounds":[{"listen":"127.0.0.1","port":port,"protocol":"vless",
            "settings":{"clients":[{"id":user}],"decryption":"none"}}],
        // Xray's server-side Freedom policy requires an explicit loopback allow.
        "outbounds":[{"protocol":"freedom","settings":{"finalRules":[{
            "action":"allow","network":"tcp","ip":["127.0.0.1/32"],
            "port":destination.port().to_string()
        }]}}]
    });
    let mut upstream = crate::engine::xray::XraySession::default();
    crate::engine::xray::start(
        &state.core.read().await,
        &mut upstream,
        &serde_json::to_vec(&config).unwrap(),
        port,
        metrics,
    )
    .await
    .unwrap();
    let node = crate::subscription::uri::parse(
        &format!("vless://{user}@127.0.0.1:{port}?security=none#Previous"),
        "rollback",
    )
    .unwrap();
    let selected = node.id.clone();
    let mut repo = Repository {
        nodes: vec![node],
        selected_server_id: Some(selected.clone()),
        ..Default::default()
    };
    repo.settings.core_engine = CoreEngine::Xray;
    repo.settings.network_mode = "proxy".into();
    repo.settings.bypass_lan = false;
    let mut session = Session::default();
    crate::engine::connect(&state, &repo, &selected, &mut session)
        .await
        .unwrap();
    drop(request(session.proxy_port(), destination).await);

    let unused = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let unavailable = unused.local_addr().unwrap();
    let rejecting_proxy = tokio::spawn(async move {
        while let Ok((stream, _)) = unused.accept().await {
            drop(stream);
        }
    });
    let next =
        crate::subscription::uri::parse(&format!("socks5://{unavailable}#Unavailable"), "rollback")
            .unwrap();
    let next_id = next.id.clone();
    repo.nodes.push(next);
    repo.settings.core_engine = CoreEngine::Lumen;
    *state.repository.write().await = repo.clone();
    let previous = session.rollback_repository(&repo);
    crate::engine::connect(&state, &repo, &next_id, &mut session)
        .await
        .unwrap();
    let mut client = TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, session.proxy_port()))
        .await
        .unwrap();
    let failure = tokio::time::timeout(
        Duration::from_secs(3),
        outbound::socks_connect(
            &mut client,
            &outbound::Destination::new(destination.ip().to_string(), destination.port()).unwrap(),
            "",
            "",
        ),
    )
    .await
    .unwrap();
    assert!(failure.is_err());
    crate::services::vpn_service::stop(&mut session)
        .await
        .unwrap();
    crate::engine::connect(&state, &previous, &selected, &mut session)
        .await
        .unwrap();
    assert_eq!(session.engine(), CoreEngine::Xray);
    drop(request(session.proxy_port(), destination).await);
    assert_eq!(
        state.repository.read().await.settings.core_engine,
        CoreEngine::Lumen
    );
    crate::services::vpn_service::stop(&mut session)
        .await
        .unwrap();
    crate::engine::xray::stop(&mut upstream).await.unwrap();
    rejecting_proxy.abort();
    origin.abort();
}
