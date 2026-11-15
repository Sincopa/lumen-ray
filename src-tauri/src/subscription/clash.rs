use crate::domain::{
    errors::{BackendError, Result},
    node::{Protocol, Security, Transport, VpnNode},
};
use serde_json::Value;

pub fn parse(v: &Value, subscription: &str) -> Result<VpnNode> {
    let s_ref = |k: &str| v[k].as_str().unwrap_or_default();
    let s = |k: &str| s_ref(k).to_owned();
    let protocol = match s_ref("type") {
        "socks5" => Protocol::Socks,
        "http" => Protocol::Http,
        "vless" => Protocol::Vless,
        "vmess" => Protocol::Vmess,
        "trojan" => Protocol::Trojan,
        "ss" => Protocol::Shadowsocks,
        "hysteria2" => Protocol::Hysteria,
        _ => return Err(BackendError::unsupported()),
    };
    // Skipped are only proxies that turn certificate checks off or that need
    // something Xray cannot run (plugins, chains, sing-box mux, legacy VMess).
    // Every other key Lumen does not model only tunes mihomo (tfo, ip-version,
    // early data, extra headers, ...) and is ignored, as other clients do.
    let object = v.as_object().ok_or_else(BackendError::invalid)?;
    let encryption = s_ref("encryption");
    if ["plugin", "plugin-opts", "dialer-proxy"]
        .iter()
        .any(|key| object.contains_key(*key))
        || v["skip-cert-verify"] == true
        || v["alterId"].as_u64().is_some_and(|n| n != 0)
        || v["smux"]["enabled"] == true
        || (protocol == Protocol::Hysteria && !["", "salamander"].contains(&s_ref("obfs")))
        || (protocol != Protocol::Vless && !["", "none"].contains(&encryption))
    {
        return Err(BackendError::unsupported());
    }
    let net_raw = s_ref("network");
    let network: String = match net_raw {
        "" if protocol == Protocol::Hysteria => "hysteria",
        "" => "raw",
        "ws" if v["ws-opts"]["v2ray-http-upgrade"] == true => "httpupgrade",
        other => other,
    }
    .into();
    if ![
        "raw",
        "tcp",
        "ws",
        "httpupgrade",
        "grpc",
        "xhttp",
        "hysteria",
    ]
    .contains(&network.as_str())
    {
        return Err(BackendError::unsupported());
    }
    let xhttp = &v["xhttp-opts"];
    let reality = v.get("reality-opts");
    let servername = s_ref("servername");
    let mut node = VpnNode {
        wireguard: None,
        balancer: None,
        outbound: None,
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
        method: if matches!(protocol, Protocol::Socks | Protocol::Http) {
            s("username")
        } else {
            s("cipher")
        },
        flow: s("flow"),
        transport: Transport {
            network,
            path: v["ws-opts"]["path"]
                .as_str()
                .or_else(|| xhttp["path"].as_str())
                .unwrap_or_default()
                .into(),
            host: v["ws-opts"]["headers"]["Host"]
                .as_str()
                .or_else(|| xhttp["host"].as_str())
                .unwrap_or_default()
                .into(),
            service_name: v["grpc-opts"]["grpc-service-name"]
                .as_str()
                .unwrap_or_default()
                .into(),
            mode: xhttp["mode"].as_str().unwrap_or_default().into(),
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
            server_name: if servername.is_empty() {
                s("sni")
            } else {
                servername.into()
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
    let obfs_password = v["obfs-password"]
        .as_str()
        .filter(|_| !s_ref("obfs").is_empty());
    let encryption = Some(encryption).filter(|value| !["", "none"].contains(value));
    // Headers besides Host (a token, say) may be what lets the server in.
    let ws_headers = v["ws-opts"]["headers"]
        .as_object()
        .filter(|headers| node.transport.network == "ws" && headers.keys().any(|k| k != "Host"))
        .cloned();
    if obfs_password.is_some() || encryption.is_some() || ws_headers.is_some() {
        super::xray_json::extend(&mut node, |outbound| {
            if let Some(encryption) = encryption {
                outbound["settings"]["vnext"][0]["users"][0]["encryption"] = encryption.into();
            }
            if let Some(headers) = ws_headers {
                outbound["streamSettings"]["wsSettings"]["headers"] = Value::Object(headers);
            }
            if let Some(password) = obfs_password {
                outbound["streamSettings"]["finalmask"] = serde_json::json!(
                    {"udp": [{"type": "salamander", "settings": {"password": password}}]}
                );
            }
        })?;
    }
    node.identify()?;
    Ok(node)
}
