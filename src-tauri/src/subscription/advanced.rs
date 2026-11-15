use crate::domain::{
    errors::{BackendError, Result},
    node::{Protocol, Security, Transport, VpnNode, WireguardOptions},
};
use serde_json::Value;
pub fn parse(v: &Value, subscription: &str) -> Result<VpnNode> {
    let wireguard = v["protocol"] == "wireguard";
    // Settings Lumen does not model keep the provider outbound (see
    // `VpnNode::outbound`) instead of dropping the server.
    let (options, mut exact) = if wireguard {
        let (options, exact) = wireguard_options(&v["settings"])?;
        (Some(options), exact)
    } else {
        (None, true)
    };
    if super::xray_json::chained(v) || v["mux"]["enabled"] == true {
        return Err(BackendError::unsupported());
    }
    exact &= v.get("mux").is_none();
    let (address, port, credential) = if let Some(wg) = &options {
        if wg.peers.is_empty() {
            return Err(BackendError::unsupported());
        }
        exact &= v.get("streamSettings").is_none();
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
        exact &= v["streamSettings"].get("finalmask").is_none()
            && v["streamSettings"]["hysteriaSettings"]
                .get("masquerade")
                .is_none();
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
        outbound: None,
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
    if !exact {
        node.outbound = Some(super::xray_json::provider_outbound(v));
    }
    node.identify()?;
    Ok(node)
}

/// WireGuard settings, and whether Lumen models all of them. Keys Lumen does not
/// know (a newer Xray option, say) are left out of the fields and kept in the
/// provider outbound.
fn wireguard_options(settings: &Value) -> Result<(WireguardOptions, bool)> {
    if let Ok(options) = serde_json::from_value(settings.clone()) {
        return Ok((options, true));
    }
    fn retain(value: &mut Value, keys: &[&str]) {
        if let Some(object) = value.as_object_mut() {
            object.retain(|key, _| keys.contains(&key.as_str()));
        }
    }
    let mut known = settings.clone();
    retain(
        &mut known,
        &[
            "secretKey",
            "address",
            "peers",
            "mtu",
            "reserved",
            "domainStrategy",
            "noKernelTun",
        ],
    );
    for peer in known["peers"].as_array_mut().into_iter().flatten() {
        retain(
            peer,
            &[
                "endpoint",
                "publicKey",
                "preSharedKey",
                "keepAlive",
                "allowedIPs",
            ],
        );
    }
    serde_json::from_value(known)
        .map(|options| (options, false))
        .map_err(|_| BackendError::unsupported())
}
