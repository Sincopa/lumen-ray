use crate::domain::{
    errors::{BackendError, Result},
    node::{Protocol, Security, Transport, VpnNode, WireguardOptions},
};
use serde_json::Value;
pub fn parse(v: &Value, subscription: &str) -> Result<VpnNode> {
    let wireguard = v["protocol"] == "wireguard";
    let options: Option<WireguardOptions> = if wireguard {
        Some(
            serde_json::from_value(v["settings"].clone())
                .map_err(|_| BackendError::unsupported())?,
        )
    } else {
        None
    };
    if v.get("proxySettings").is_some() || v.get("mux").is_some() {
        return Err(BackendError::unsupported());
    }
    let (address, port, credential) = if let Some(wg) = &options {
        if v.get("streamSettings").is_some() || wg.peers.is_empty() {
            return Err(BackendError::unsupported());
        }
        let endpoint = url::Url::parse(&format!("udp://{}", wg.peers[0].endpoint))
            .map_err(|_| BackendError::invalid())?;
        (
            endpoint
                .host_str()
                .ok_or_else(BackendError::invalid)?
                .trim_matches(['[', ']'])
                .into(),
            endpoint.port().ok_or_else(BackendError::invalid)?,
            wg.secret_key.clone(),
        )
    } else {
        if v["settings"]["version"] != 2 || v["streamSettings"]["hysteriaSettings"]["version"] != 2
        {
            return Err(BackendError::unsupported());
        }
        if v["streamSettings"].get("finalmask").is_some()
            || v["streamSettings"]["hysteriaSettings"]
                .get("masquerade")
                .is_some()
        {
            return Err(BackendError::unsupported());
        }
        (
            v["settings"]["address"]
                .as_str()
                .ok_or_else(BackendError::invalid)?
                .into(),
            v["settings"]["port"]
                .as_u64()
                .and_then(|n| u16::try_from(n).ok())
                .ok_or_else(BackendError::invalid)?,
            v["streamSettings"]["hysteriaSettings"]["auth"]
                .as_str()
                .unwrap_or_default()
                .into(),
        )
    };
    let tls = &v["streamSettings"]["tlsSettings"];
    if tls["allowInsecure"] == true || tls.get("certificates").is_some() {
        return Err(BackendError::unsupported());
    }
    let mut node = VpnNode {
        id: String::new(),
        subscription_id: subscription.into(),
        name: v["remarks"]
            .as_str()
            .or_else(|| v["tag"].as_str())
            .unwrap_or("Server")
            .into(),
        address,
        port,
        credential,
        protocol: if wireguard {
            Protocol::Wireguard
        } else {
            Protocol::Hysteria
        },
        method: String::new(),
        flow: String::new(),
        wireguard: options,
        balancer: None,
        transport: Transport {
            network: if wireguard { "raw" } else { "hysteria" }.into(),
            ..Default::default()
        },
        security: Security {
            kind: if wireguard { "none" } else { "tls" }.into(),
            server_name: tls["serverName"].as_str().unwrap_or_default().into(),
            alpn: tls["alpn"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|s| s.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default(),
            ..Default::default()
        },
    };
    node.identify()?;
    Ok(node)
}
