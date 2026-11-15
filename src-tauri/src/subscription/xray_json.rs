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
/// Reads an Xray outbound. A server is skipped only when the core cannot run
/// it on its own or when it asks to turn certificate checks off. Settings Lumen
/// does not model keep the provider's outbound as is (see `VpnNode::outbound`).
pub fn parse(v: &Value, subscription: &str) -> Result<VpnNode> {
    if [Some("hysteria"), Some("wireguard")].contains(&v["protocol"].as_str()) {
        return super::advanced::parse(v, subscription);
    }
    let protocol = match v["protocol"].as_str() {
        Some("socks") => Protocol::Socks,
        Some("http") => Protocol::Http,
        Some("vless") => Protocol::Vless,
        Some("vmess") => Protocol::Vmess,
        Some("trojan") => Protocol::Trojan,
        Some("shadowsocks") => Protocol::Shadowsocks,
        _ => return Err(BackendError::unsupported()),
    };
    if chained(v) {
        return Err(BackendError::unsupported());
    }
    let vnext = matches!(protocol, Protocol::Vless | Protocol::Vmess);
    let settings = &v["settings"];
    let list = if vnext {
        &settings["vnext"]
    } else {
        &settings["servers"]
    };
    // Xray also takes the server inline: `settings: {address, port, id, ...}`.
    let endpoint = match list.as_array() {
        Some(list) => list.first().ok_or_else(BackendError::unsupported)?,
        None if settings.get("address").is_some() => settings,
        None => return Err(BackendError::unsupported()),
    };
    let user = match endpoint.get("users") {
        Some(users) if vnext => users
            .as_array()
            .and_then(|users| users.first())
            .ok_or_else(BackendError::unsupported)?,
        _ => endpoint,
    };
    let proxy_auth = if matches!(protocol, Protocol::Socks | Protocol::Http) {
        match endpoint.get("users") {
            Some(users) => users
                .as_array()
                .ok_or_else(BackendError::unsupported)?
                .first()
                .unwrap_or(&Value::Null),
            None => &Value::Null,
        }
    } else {
        &Value::Null
    };
    let stream = &v["streamSettings"];
    let security = stream["security"].as_str().unwrap_or("none");
    let sec = if security == "reality" {
        &stream["realitySettings"]
    } else {
        &stream["tlsSettings"]
    };
    // Legacy VMess (alterId) is gone from Xray; an insecure server stays out.
    if sec["allowInsecure"] == true || user["alterId"].as_u64().is_some_and(|n| n != 0) {
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
    let mut node = VpnNode {
        wireguard: None,
        balancer: None,
        outbound: None,
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
        credential: if matches!(protocol, Protocol::Socks | Protocol::Http) {
            proxy_auth["pass"].as_str().unwrap_or_default().into()
        } else {
            user[if vnext { "id" } else { "password" }]
                .as_str()
                .ok_or_else(BackendError::invalid)?
                .into()
        },
        protocol,
        method: if matches!(protocol, Protocol::Socks | Protocol::Http) {
            proxy_auth["user"].as_str().unwrap_or_default().into()
        } else {
            user["method"]
                .as_str()
                .or_else(|| user["security"].as_str())
                .unwrap_or("auto")
                .into()
        },
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
    if !modeled(v, protocol) {
        node.outbound = Some(provider_outbound(v));
    }
    node.identify()?;
    Ok(node)
}

/// An outbound that dials through another outbound by tag. Alone, without
/// that outbound, the core would not start.
pub(super) fn chained(v: &Value) -> bool {
    v.get("proxySettings").is_some() || !v["streamSettings"]["sockopt"]["dialerProxy"].is_null()
}

/// The provider's outbound as the core runs it. Lumen names outbounds itself,
/// and a local source address belongs to the provider's machine, not this one.
pub(super) fn provider_outbound(v: &Value) -> Value {
    let mut outbound = v.clone();
    if let Some(object) = outbound.as_object_mut() {
        for key in ["tag", "remarks", "sendThrough"] {
            object.remove(key);
        }
    }
    outbound
}

/// Keeps the outbound built from the node's fields, plus what `patch` adds
/// that the fields do not model, as the node's provider outbound.
pub(super) fn extend(node: &mut VpnNode, patch: impl FnOnce(&mut Value)) -> Result<()> {
    let mut outbound = crate::xray::config::outbound(node, "")?;
    patch(&mut outbound);
    node.outbound = Some(provider_outbound(&outbound));
    Ok(())
}

/// Whether Lumen's fields describe the outbound completely, so the outbound
/// built from them is the same server. Everything the parser accepted before
/// it kept provider outbounds still counts as modeled: those servers keep
/// their ids. (A refresh also keeps an id by address, port and name.)
fn modeled(v: &Value, protocol: Protocol) -> bool {
    let only = |value: &Value, allowed: &[&str]| keys(value, allowed).is_ok();
    let vnext = matches!(protocol, Protocol::Vless | Protocol::Vmess);
    let list_key = if vnext { "vnext" } else { "servers" };
    if !only(
        v,
        &[
            "tag",
            "protocol",
            "settings",
            "streamSettings",
            "mux",
            "remarks",
            "sendThrough",
        ],
    ) || !only(&v["settings"], &[list_key])
    {
        return false;
    }
    let Some([endpoint]) = v["settings"][list_key].as_array().map(Vec::as_slice) else {
        return false;
    };
    if vnext {
        let Some([user]) = endpoint["users"].as_array().map(Vec::as_slice) else {
            return false;
        };
        if !only(endpoint, &["address", "port", "users"])
            || !only(
                user,
                &[
                    "id",
                    "encryption",
                    "flow",
                    "security",
                    "alterId",
                    "level",
                    "email",
                ],
            )
            || user["encryption"].as_str().is_some_and(|s| s != "none")
        {
            return false;
        }
    } else if !only(
        endpoint,
        &[
            "address", "port", "password", "method", "users", "level", "email",
        ],
    ) || endpoint["users"]
        .as_array()
        .is_some_and(|users| users.len() > 1)
    {
        return false;
    }
    if v.get("mux").is_some_and(|m| m["enabled"] == true) {
        return false;
    }
    let stream = &v["streamSettings"];
    let sec = if stream["security"] == "reality" {
        &stream["realitySettings"]
    } else {
        &stream["tlsSettings"]
    };
    if stream.get("sockopt").is_some()
        || !only(
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
        )
        || !only(
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
        )
    {
        return false;
    }
    match stream["network"].as_str().unwrap_or("raw") {
        "ws" => {
            only(&stream["wsSettings"], &["path", "headers"])
                && only(&stream["wsSettings"]["headers"], &["Host"])
        }
        "grpc" => {
            let grpc = &stream["grpcSettings"];
            only(grpc, &["serviceName", "multiMode", "authority", "mode"])
                && grpc["mode"]
                    .as_str()
                    .is_none_or(|mode| ["", "gun", "multi"].contains(&mode))
        }
        "xhttp" => only(&stream["xhttpSettings"], &["path", "host", "mode"]),
        "httpupgrade" => only(&stream["httpupgradeSettings"], &["path", "host"]),
        "kcp" | "mkcp" => {
            only(&stream["kcpSettings"], &["seed", "header"])
                && only(&stream["kcpSettings"]["header"], &["type"])
        }
        _ => ["rawSettings", "tcpSettings"].iter().all(|key| {
            only(&stream[*key], &["header"])
                && stream[*key]["header"]["type"]
                    .as_str()
                    .is_none_or(|kind| kind == "none")
        }),
    }
}
