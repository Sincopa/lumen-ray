use super::*;

#[test]
fn interrupted_repository_write_recovers_previous_encrypted_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let store = RepositoryStore::new(dir.path()).unwrap();
    let mut repo = Repository::default();
    repo.settings.auto_connect = true;
    store.save(&repo).unwrap();
    repo.settings.close_to_tray = false;
    store.save(&repo).unwrap();
    std::fs::write(dir.path().join("repository.dpapi"), b"interrupted write").unwrap();
    let recovered = store.load().unwrap();
    assert!(recovered.settings.close_to_tray);
    assert!(!recovered.settings.auto_connect);
    store.save(&recovered).unwrap();
    assert!(store.load().is_ok());
}

#[cfg(windows)]
#[tokio::test]
async fn native_local_proxy_smoke() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let dir = tempfile::tempdir().unwrap();
    let runtime = crate::runtime::RuntimePaths::new(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("runtime"),
    );
    let core = bundle::prepare(&runtime, dir.path()).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let origin = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0u8; 4096];
        let read = socket.read(&mut request).await.unwrap();
        assert!(String::from_utf8_lossy(&request[..read]).starts_with("GET /native-proof "));
        socket
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Length: 12\r\nConnection: close\r\n\r\nlumen-native",
            )
            .await
            .unwrap();
    });
    let (proxy, metrics) = crate::engine::xray::ports().unwrap();
    let bytes = serde_json::to_vec(&serde_json::json!({
        "log":{"loglevel":"none"},
        "inbounds":[{"listen":"127.0.0.1","port":proxy,"protocol":"socks","settings":{"auth":"noauth"}}],
        "outbounds":[{"protocol":"freedom"}]
    })).unwrap();
    let mut session = crate::engine::xray::XraySession::default();
    crate::engine::xray::start(&core, &mut session, &bytes, proxy, metrics)
        .await
        .unwrap();
    let result = crate::services::health::client(Some(proxy))
        .unwrap()
        .get(format!("http://{address}/native-proof"))
        .send()
        .await;
    // Cleanup is unconditional even if the network assertion fails.
    let body = match result {
        Ok(response) => response.text().await,
        Err(error) => Err(error),
    };
    crate::engine::xray::stop(&mut session).await.unwrap();
    assert_eq!(body.unwrap(), "lumen-native");
    tokio::time::timeout(std::time::Duration::from_secs(5), origin)
        .await
        .unwrap()
        .unwrap();
    assert!(
        tokio::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, proxy))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn split_dns_is_accepted_by_bundled_core() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = crate::runtime::RuntimePaths::new(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("runtime"),
    );
    let core = bundle::prepare(&runtime, dir.path()).unwrap();
    let node = uri::parse(VLESS, "test").unwrap();
    let mut repo = Repository {
        nodes: vec![node.clone()],
        ..Default::default()
    };
    repo.settings.dns_policy.enabled = true;
    repo.settings.dns_policy.direct = "9.9.9.9".into();
    repo.settings.dns_policy.proxy = "https://dns.example.org/dns-query".into();
    repo.settings.dns_policy.fallback = vec!["8.8.8.8".into()];
    for mode in ["global", "rules"] {
        for ipv6 in ["auto", "ipv4"] {
            for tun in [false, true] {
                repo.routing_mode = mode.into();
                repo.settings.dns_policy.ipv6 = ipv6.into();
                let config = config::build(&repo, &node.id, 10808, 10809, tun).unwrap();
                crate::engine::xray::validate(&core, &serde_json::to_vec(&config).unwrap())
                    .await
                    .unwrap();
            }
        }
    }
}
