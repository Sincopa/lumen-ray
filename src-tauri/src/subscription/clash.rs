use crate::domain::{
    errors::{BackendError, Result},
    node::{Protocol, Security, Transport, VpnNode},
};
use serde_json::Value;

pub fn parse(v: &Value, subscription: &str) -> Result<VpnNode> {
    let s = |k: &str| v[k].as_str().unwrap_or_default().to_owned();
    let protocol = match s("type").as_str() {
        "vless" => Protocol::Vless,
        "vmess" => Protocol::Vmess,
        "trojan" => Protocol::Trojan,
        "ss" => Protocol::Shadowsocks,
        "hysteria2" => Protocol::Hysteria,
        _ => return Err(BackendError::unsupported()),
    };
    let allowed = [
        "name",
        "type",
        "server",
        "port",
        "uuid",
        "password",
        "cipher",
        "alterId",
        "tls",
        "servername",
        "sni",
        "client-fingerprint",
        "reality-opts",
        "network",
        "ws-opts",
        "grpc-opts",
        "flow",
        "udp",
        "skip-cert-verify",
        "alpn",
    ];
    if v.as_object()
        .ok_or_else(BackendError::invalid)?
        .keys()
        .any(|k| !allowed.contains(&k.as_str()))
        || v["skip-cert-verify"] == true
        || v["alterId"].as_u64().is_some_and(|n| n != 0)
    {
        return Err(BackendError::unsupported());
    }
    super::xray_json::keys(&v["ws-opts"], &["path", "headers"])?;
    super::xray_json::keys(&v["ws-opts"]["headers"], &["Host"])?;
    super::xray_json::keys(&v["grpc-opts"], &["grpc-service-name"])?;
    super::xray_json::keys(&v["reality-opts"], &["public-key", "short-id"])?;
    let network = if s("network").is_empty() {
        if protocol == Protocol::Hysteria {
            "hysteria"
        } else {
            "raw"
        }
        .into()
    } else {
        s("network")
    };
    if !["raw", "tcp", "ws", "grpc", "hysteria"].contains(&network.as_str()) {
        return Err(BackendError::unsupported());
    }
    let reality = v.get("reality-opts");
    let mut node = VpnNode {
        wireguard: None,
        balancer: None,
        id: String::new(),
        subscription_id: subscription.into(),
        name: s("name"),
        address: s("server"),
        port: v["port"]
            .as_u64()
            .and_then(|n| u16::try_from(n).ok())
            .ok_or_else(BackendError::invalid)?,
        credential: if matches!(protocol, Protocol::Vless | Protocol::Vmess) {
            s("uuid")
        } else {
            s("password")
        },
        protocol,
        method: s("cipher"),
        flow: s("flow"),
        transport: Transport {
            network,
            path: v["ws-opts"]["path"].as_str().unwrap_or_default().into(),
            host: v["ws-opts"]["headers"]["Host"]
                .as_str()
                .unwrap_or_default()
                .into(),
            service_name: v["grpc-opts"]["grpc-service-name"]
                .as_str()
                .unwrap_or_default()
                .into(),
            mode: String::new(),
            ..Transport::default()
        },
        security: Security {
            spider_x: String::new(),
            kind: if reality.is_some() {
                "reality"
            } else if v["tls"] == true || matches!(protocol, Protocol::Trojan | Protocol::Hysteria)
            {
                "tls"
            } else {
                "none"
            }
            .into(),
            server_name: if s("servername").is_empty() {
                s("sni")
            } else {
                s("servername")
            },
            fingerprint: s("client-fingerprint"),
            public_key: v["reality-opts"]["public-key"]
                .as_str()
                .unwrap_or_default()
                .into(),
            short_id: v["reality-opts"]["short-id"]
                .as_str()
                .unwrap_or_default()
                .into(),
            alpn: v["alpn"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|s| s.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default(),
        },
    };
    node.identify()?;
    Ok(node)
}
