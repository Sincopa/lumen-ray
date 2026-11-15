use crate::domain::{
    errors::{BackendError, Result},
    node::{Protocol, Security, Transport, VpnNode},
};
use base64::{
    engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD},
    Engine,
};
use percent_encoding::percent_decode_str;
use serde_json::json;
use std::collections::BTreeMap;
use url::Url;

pub fn decode(input: &str) -> Result<String> {
    let trimmed = input.trim();
    let filtered: String;
    let clean = if trimmed.chars().any(char::is_whitespace) {
        filtered = trimmed.chars().filter(|c| !c.is_whitespace()).collect();
        filtered.as_str()
    } else {
        trimmed
    };
    [STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD]
        .iter()
        .find_map(|engine| engine.decode(clean).ok())
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .ok_or_else(BackendError::invalid)
}
fn unescape(input: &str) -> Result<String> {
    percent_decode_str(input)
        .decode_utf8()
        .map(|s| s.into_owned())
        .map_err(|_| BackendError::invalid())
}

pub fn parse(input: &str, subscription: &str) -> Result<VpnNode> {
    if let Some(encoded) = input.strip_prefix("vmess://") {
        return vmess(encoded, subscription);
    }
    let ss_decoded: String;
    let normalized = if let Some(encoded) = input.strip_prefix("ss://") {
        if !encoded.contains('@') {
            let (body, fragment) = encoded.split_once('#').unwrap_or((encoded, ""));
            ss_decoded = format!("ss://{}#{}", decode(body)?, fragment);
            ss_decoded.as_str()
        } else {
            input
        }
    } else {
        input
    };
    let url = Url::parse(normalized).map_err(|_| BackendError::invalid())?;
    let protocol = match url.scheme() {
        "socks" | "socks5" => Protocol::Socks,
        "http-proxy" => Protocol::Http,
        "vless" => Protocol::Vless,
        "trojan" => Protocol::Trojan,
        "ss" => Protocol::Shadowsocks,
        "hysteria2" | "hy2" => Protocol::Hysteria,
        _ => return Err(BackendError::unsupported()),
    };
    let query: BTreeMap<String, String> = url
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    let q_str = |key: &str| query.get(key).map(String::as_str).unwrap_or_default();
    let q = |key: &str| query.get(key).cloned().unwrap_or_default();
    // A link is never skipped for a key Lumen does not know: like every client,
    // Lumen ignores hints meant for others, and Lumen's own defaults are the
    // strict ones (certificates are checked). The keys Xray needs and the
    // fields do not model go into the server's provider outbound below. Skipped
    // are only links that turn certificate checks off and settings Xray cannot run.
    let encryption = q_str("encryption");
    let header_type = q_str("headerType");
    let kcp = matches!(q_str("type"), "kcp" | "mkcp");
    let http_header = header_type == "http" && matches!(q_str("type"), "" | "raw" | "tcp") && !kcp;
    if ["allowInsecure", "insecure"]
        .iter()
        .any(|k| ["1", "true"].contains(&q_str(k)))
        || (!kcp && !http_header && !["", "none"].contains(&header_type))
        || (protocol != Protocol::Vless && !["", "none"].contains(&encryption))
        || (protocol == Protocol::Hysteria && !["", "salamander"].contains(&q_str("obfs")))
    {
        return Err(BackendError::unsupported());
    }
    let extra = match q_str("extra") {
        "" => None,
        raw => Some(
            serde_json::from_str::<serde_json::Value>(raw)
                .ok()
                .filter(serde_json::Value::is_object)
                .ok_or_else(BackendError::invalid)?,
        ),
    };
    let mut credential = unescape(url.username())?;
    let mut method = String::new();
    if protocol == Protocol::Shadowsocks {
        let auth = if let Some(password) = url.password() {
            format!("{}:{}", credential, unescape(password)?)
        } else {
            decode(&credential)?
        };
        let (m, p) = auth.split_once(':').ok_or_else(BackendError::invalid)?;
        method = m.to_owned();
        credential = p.to_owned();
    }
    if matches!(protocol, Protocol::Socks | Protocol::Http) {
        method = credential;
        credential = unescape(url.password().unwrap_or_default())?;
        if method.is_empty() != credential.is_empty() {
            return Err(BackendError::invalid());
        }
    }
    let default_security = if matches!(protocol, Protocol::Trojan | Protocol::Hysteria) {
        "tls"
    } else {
        "none"
    };
    let mut node = VpnNode {
        wireguard: None,
        balancer: None,
        outbound: None,
        id: String::new(),
        subscription_id: subscription.into(),
        name: unescape(url.fragment().unwrap_or("Server"))?,
        address: url
            .host_str()
            .ok_or_else(BackendError::invalid)?
            .trim_matches(['[', ']'])
            .to_owned(),
        port: url.port().ok_or_else(BackendError::invalid)?,
        protocol,
        credential,
        method,
        flow: q("flow"),
        transport: Transport {
            network: query.get("type").cloned().unwrap_or_else(|| {
                if protocol == Protocol::Hysteria {
                    "hysteria"
                } else {
                    "raw"
                }
                .into()
            }),
            path: q("path"),
            host: q("host"),
            service_name: q("serviceName"),
            mode: q("mode"),
            seed: q("seed"),
            header_type: q("headerType"),
        },
        security: Security {
            spider_x: q("spx"),
            kind: query
                .get("security")
                .cloned()
                .unwrap_or_else(|| default_security.into()),
            server_name: query
                .get("sni")
                .or_else(|| query.get("peer"))
                .cloned()
                .unwrap_or_default(),
            fingerprint: q("fp"),
            public_key: q("pbk"),
            short_id: q("sid"),
            alpn: q_str("alpn")
                .split(',')
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect(),
        },
    };
    let set = |key: &str| Some(q_str(key)).filter(|value| !value.is_empty());
    let (pqv, ech, authority) = (set("pqv"), set("ech"), set("authority"));
    let obfs_password = set("obfs-password").filter(|_| set("obfs").is_some());
    let encryption = Some(encryption).filter(|value| !["", "none"].contains(value));
    if http_header
        || extra.is_some()
        || encryption.is_some()
        || obfs_password.is_some()
        || pqv.is_some()
        || ech.is_some()
        || authority.is_some()
    {
        let (path, host) = (node.transport.path.clone(), node.transport.host.clone());
        super::xray_json::extend(&mut node, |outbound| {
            if let Some(encryption) = encryption {
                outbound["settings"]["vnext"][0]["users"][0]["encryption"] = json!(encryption);
            }
            let stream = &mut outbound["streamSettings"];
            if http_header {
                let path = if path.is_empty() { "/".into() } else { path };
                let hosts: Vec<&str> = host.split(',').filter(|h| !h.is_empty()).collect();
                stream["rawSettings"] = json!({"header": {"type": "http", "request": {
                    "path": [path],
                    "headers": if hosts.is_empty() { json!({}) } else { json!({"Host": hosts}) },
                }}});
            }
            if let Some(extra) = extra.filter(|_| stream["network"] == "xhttp") {
                stream["xhttpSettings"]["extra"] = extra;
            }
            if let Some(password) = obfs_password {
                stream["finalmask"] =
                    json!({"udp": [{"type": "salamander", "settings": {"password": password}}]});
            }
            if let Some(pqv) = pqv.filter(|_| stream["security"] == "reality") {
                stream["realitySettings"]["mldsa65Verify"] = json!(pqv);
            }
            if let Some(ech) = ech.filter(|_| stream["security"] == "tls") {
                stream["tlsSettings"]["echConfigList"] = json!(ech);
            }
            if let Some(authority) = authority.filter(|_| stream["network"] == "grpc") {
                stream["grpcSettings"]["authority"] = json!(authority);
            }
        })?;
    }
    node.identify()?;
    Ok(node)
}
fn vmess(input: &str, subscription: &str) -> Result<VpnNode> {
    let value: serde_json::Value =
        serde_json::from_str(&decode(input)?).map_err(|_| BackendError::invalid())?;
    let s = |k: &str| {
        value[k]
            .as_str()
            .map(str::to_owned)
            .or_else(|| value[k].as_u64().map(|n| n.to_string()))
            .unwrap_or_default()
    };
    let aid = s("aid");
    let vtype = s("type");
    if !["", "0"].contains(&aid.as_str()) || !["", "none"].contains(&vtype.as_str()) {
        return Err(BackendError::unsupported());
    }
    let scy = s("scy");
    let net = s("net");
    let path = s("path");
    let tls = s("tls");
    let alpn_raw = s("alpn");
    let mut node = VpnNode {
        wireguard: None,
        balancer: None,
        outbound: None,
        id: String::new(),
        subscription_id: subscription.into(),
        name: s("ps"),
        address: s("add"),
        port: s("port").parse().map_err(|_| BackendError::invalid())?,
        protocol: Protocol::Vmess,
        credential: s("id"),
        method: if scy.is_empty() { "auto".into() } else { scy },
        flow: String::new(),
        transport: Transport {
            network: if net.is_empty() { "raw".into() } else { net },
            service_name: path.clone(),
            path,
            host: s("host"),
            mode: String::new(),
            ..Transport::default()
        },
        security: Security {
            kind: if tls.is_empty() { "none".into() } else { tls },
            server_name: s("sni"),
            fingerprint: s("fp"),
            alpn: alpn_raw
                .split(',')
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect(),
            ..Security::default()
        },
    };
    node.identify()?;
    Ok(node)
}
