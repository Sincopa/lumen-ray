use crate::domain::{
    errors::{BackendError, Result},
    node::{Protocol, Security, Transport, VpnNode},
};
use serde_json::Value;
pub(super) fn keys(value: &Value, allowed: &[&str]) -> Result<()> {
    if value.is_null() {
        return Ok(());
    }
    if value
        .as_object()
        .ok_or_else(BackendError::invalid)?
        .keys()
        .any(|k| !allowed.contains(&k.as_str()))
    {
        return Err(BackendError::unsupported());
    }
    Ok(())
}
pub fn parse(v: &Value, subscription: &str) -> Result<VpnNode> {
    if [Some("hysteria"), Some("wireguard")].contains(&v["protocol"].as_str()) {
        return super::advanced::parse(v, subscription);
    }
    let protocol = match v["protocol"].as_str() {
        Some("vless") => Protocol::Vless,
        Some("vmess") => Protocol::Vmess,
        Some("trojan") => Protocol::Trojan,
        Some("shadowsocks") => Protocol::Shadowsocks,
        _ => return Err(BackendError::unsupported()),
    };
    if v.get("proxySettings").is_some() || v.get("mux").is_some_and(|m| m["enabled"] == true) {
        return Err(BackendError::unsupported());
    }
    let endpoints = if matches!(protocol, Protocol::Vless | Protocol::Vmess) {
        &v["settings"]["vnext"]
    } else {
        &v["settings"]["servers"]
    };
    let endpoints = endpoints
        .as_array()
        .filter(|a| a.len() == 1)
        .ok_or_else(BackendError::unsupported)?;
    let endpoint = &endpoints[0];
    let user = if matches!(protocol, Protocol::Vless | Protocol::Vmess) {
        endpoint["users"]
            .as_array()
            .filter(|a| a.len() == 1)
            .and_then(|a| a.first())
            .ok_or_else(BackendError::unsupported)?
    } else {
        endpoint
    };
    let stream = &v["streamSettings"];
    keys(
        stream,
        &[
            "network",
            "security",
            "tlsSettings",
            "realitySettings",
            "rawSettings",
            "tcpSettings",
            "wsSettings",
            "grpcSettings",
            "xhttpSettings",
            "httpupgradeSettings",
            "kcpSettings",
        ],
    )?;
    let security = stream["security"].as_str().unwrap_or("none");
    let sec = if security == "reality" {
        &stream["realitySettings"]
    } else {
        &stream["tlsSettings"]
    };
    if sec["allowInsecure"] == true || stream.get("sockopt").is_some() {
        return Err(BackendError::unsupported());
    }
    keys(
        sec,
        &[
            "serverName",
            "fingerprint",
            "password",
            "publicKey",
            "shortId",
            "alpn",
            "allowInsecure",
            "show",
            "spiderX",
        ],
    )?;
    if user["encryption"].as_str().is_some_and(|s| s != "none")
        || user["alterId"].as_u64().is_some_and(|n| n != 0)
    {
        return Err(BackendError::unsupported());
    }
    let network = stream["network"].as_str().unwrap_or("raw");
    let settings = match network {
        "ws" => &stream["wsSettings"],
        "grpc" => &stream["grpcSettings"],
        "xhttp" => &stream["xhttpSettings"],
        "httpupgrade" => &stream["httpupgradeSettings"],
        "kcp" | "mkcp" => &stream["kcpSettings"],
        "raw" | "tcp" => &Value::Null,
        _ => return Err(BackendError::unsupported()),
    };
    match network {
        "ws" => {
            keys(settings, &["path", "headers"])?;
            keys(&settings["headers"], &["Host"])?;
        }
        "grpc" => {
            keys(settings, &["serviceName", "multiMode", "authority", "mode"])?;
            if settings["mode"]
                .as_str()
                .is_some_and(|mode| !["", "gun", "multi"].contains(&mode))
            {
                return Err(BackendError::unsupported());
            }
        }
        "xhttp" => keys(settings, &["path", "host", "mode"])?,
        "httpupgrade" => keys(settings, &["path", "host"])?,
        "kcp" | "mkcp" => {
            keys(settings, &["seed", "header"])?;
            keys(&settings["header"], &["type"])?;
        }
        "raw" | "tcp" => {
            for key in ["rawSettings", "tcpSettings"] {
                keys(&stream[key], &["header"])?;
                if stream[key]["header"]["type"]
                    .as_str()
                    .is_some_and(|s| s != "none")
                {
                    return Err(BackendError::unsupported());
                }
            }
        }
        _ => {}
    }
    let mut node = VpnNode {
        wireguard: None,
        balancer: None,
        id: String::new(),
        subscription_id: subscription.into(),
        name: v["remarks"]
            .as_str()
            .or_else(|| v["tag"].as_str())
            .unwrap_or("Server")
            .into(),
        address: endpoint["address"]
            .as_str()
            .ok_or_else(BackendError::invalid)?
            .into(),
        port: endpoint["port"]
            .as_u64()
            .and_then(|p| u16::try_from(p).ok())
            .ok_or_else(BackendError::invalid)?,
        credential: user[if matches!(protocol, Protocol::Vless | Protocol::Vmess) {
            "id"
        } else {
            "password"
        }]
        .as_str()
        .ok_or_else(BackendError::invalid)?
        .into(),
        protocol,
        method: user["method"]
            .as_str()
            .or_else(|| user["security"].as_str())
            .unwrap_or("auto")
            .into(),
        flow: user["flow"].as_str().unwrap_or_default().into(),
        transport: Transport {
            network: network.into(),
            path: settings["path"].as_str().unwrap_or_default().into(),
            host: settings["host"]
                .as_str()
                .or_else(|| settings["headers"]["Host"].as_str())
                .or_else(|| settings["authority"].as_str())
                .unwrap_or_default()
                .into(),
            service_name: settings["serviceName"].as_str().unwrap_or_default().into(),
            mode: if settings["multiMode"] == true {
                "multi"
            } else {
                settings["mode"].as_str().unwrap_or_default()
            }
            .into(),
            seed: settings["seed"].as_str().unwrap_or_default().into(),
            header_type: settings["header"]["type"]
                .as_str()
                .unwrap_or_default()
                .into(),
        },
        security: Security {
            spider_x: sec["spiderX"].as_str().unwrap_or_default().into(),
            kind: security.into(),
            server_name: sec["serverName"].as_str().unwrap_or_default().into(),
            fingerprint: sec["fingerprint"].as_str().unwrap_or_default().into(),
            public_key: sec["password"]
                .as_str()
                .or_else(|| sec["publicKey"].as_str())
                .unwrap_or_default()
                .into(),
            short_id: sec["shortId"].as_str().unwrap_or_default().into(),
            alpn: sec["alpn"]
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
