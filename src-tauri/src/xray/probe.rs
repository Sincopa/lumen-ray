//! The local SOCKS inbound (`probe`) that Lumen's own checks use: health,
//! latency, public IP, Discord voice and subscription downloads through the VPN.
//! On Android every app can reach 127.0.0.1, and apps scan it for proxy ports
//! to detect VPN clients, so there the inbound requires credentials generated
//! for this run. Desktop keeps the unauthenticated loopback inbound.
use serde_json::{json, Value};

#[cfg(target_os = "android")]
fn credentials() -> Option<&'static (String, String)> {
    static CREDENTIALS: std::sync::OnceLock<(String, String)> = std::sync::OnceLock::new();
    Some(CREDENTIALS.get_or_init(|| {
        (
            uuid::Uuid::new_v4().simple().to_string(),
            uuid::Uuid::new_v4().simple().to_string(),
        )
    }))
}

#[cfg(not(target_os = "android"))]
fn credentials() -> Option<&'static (String, String)> {
    None
}

/// Settings of the `probe` inbound.
pub fn inbound_settings() -> Value {
    match credentials() {
        Some((user, pass)) => {
            json!({"auth":"password","accounts":[{"user":user,"pass":pass}],"udp":true})
        }
        None => json!({"auth":"noauth","udp":true}),
    }
}

/// A reqwest proxy for the probe inbound on `port`.
pub fn proxy(port: u16) -> reqwest::Result<reqwest::Proxy> {
    let proxy = reqwest::Proxy::all(format!("socks5h://127.0.0.1:{port}"))?;
    Ok(match credentials() {
        Some((user, pass)) => proxy.basic_auth(user, pass),
        None => proxy,
    })
}

/// SOCKS5 method negotiation on a raw connection to the probe inbound.
pub async fn handshake(stream: &mut tokio::net::TcpStream) -> Option<()> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let Some((user, pass)) = credentials() else {
        stream.write_all(&[5, 1, 0]).await.ok()?;
        let mut greeting = [0u8; 2];
        stream.read_exact(&mut greeting).await.ok()?;
        return (greeting == [5, 0]).then_some(());
    };
    stream.write_all(&[5, 1, 2]).await.ok()?;
    let mut greeting = [0u8; 2];
    stream.read_exact(&mut greeting).await.ok()?;
    if greeting != [5, 2] {
        return None;
    }
    // RFC 1929 username/password sub-negotiation.
    let mut request = vec![1, u8::try_from(user.len()).ok()?];
    request.extend_from_slice(user.as_bytes());
    request.push(u8::try_from(pass.len()).ok()?);
    request.extend_from_slice(pass.as_bytes());
    stream.write_all(&request).await.ok()?;
    let mut reply = [0u8; 2];
    stream.read_exact(&mut reply).await.ok()?;
    (reply == [1, 0]).then_some(())
}
