use crate::domain::{
    errors::{BackendError, Result},
    node::{Protocol, Security, Transport, VpnNode},
};
use base64::{
    engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD},
    Engine,
};
use percent_encoding::percent_decode_str;
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
    // Unknown knobs must not silently weaken security or change the requested transport.
    const KEYS: &[&str] = &[
        "type",
        "security",
        "sni",
        "peer",
        "fp",
        "pbk",
        "sid",
        "alpn",
        "flow",
        "encryption",
        "path",
        "host",
        "serviceName",
        "mode",
        "headerType",
        "allowInsecure",
        "insecure",
        "seed",
    ];
    if query.keys().any(|k| !KEYS.contains(&k.as_str()))
        || ["allowInsecure", "insecure"]
            .iter()
            .any(|k| ["1", "true"].contains(&q_str(k)))
        || (!matches!(q_str("type"), "kcp" | "mkcp")
            && !["", "none"].contains(&q_str("headerType")))
        || !["", "none"].contains(&q_str("encryption"))
    {
        return Err(BackendError::unsupported());
    }
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
            spider_x: String::new(),
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
