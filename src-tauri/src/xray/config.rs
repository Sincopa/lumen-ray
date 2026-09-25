use crate::{
    domain::{
        errors::{BackendError, Result},
        node::{Protocol, VpnNode},
    },
    storage::repository::Repository,
};
use serde::Serialize;
use serde_json::{json, Value};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XrayConfig {
    log: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    api: Option<Value>,
    inbounds: Vec<Value>,
    outbounds: Vec<Value>,
    routing: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    dns: Option<Value>,
    stats: Value,
    policy: Value,
    metrics: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    observatory: Option<Value>,
}

#[cfg(windows)]
pub fn build_standby(
    repo: &Repository,
    selected: &str,
    proxy_port: u16,
    metrics_port: u16,
) -> Result<XrayConfig> {
    let mut config = build(repo, selected, proxy_port, metrics_port, true)?;
    let tun = config.inbounds.iter_mut()
        .find(|inbound| inbound["protocol"] == "tun")
        .ok_or_else(BackendError::invalid)?;
    tun["settings"].as_object_mut()
        .ok_or_else(BackendError::invalid)?
        .remove("autoSystemRoutingTable");
    let api_port = metrics_port.checked_add(2).ok_or_else(BackendError::invalid)?;
    config.api = Some(json!({"tag":"lumen-api","listen":format!("127.0.0.1:{api_port}"),"services":["RoutingService"]}));
    Ok(config)
}

/// Xray replaces the complete rule set atomically through RoutingService.
/// Other configuration sections must stay identical during a live update.
#[cfg(windows)]
pub fn live_routing_patch(repo_before: &Repository, repo_after: &Repository, selected: &str) -> Result<Option<Value>> {
    let before = build(repo_before, selected, 10808, 10809, true)?;
    let after = build(repo_after, selected, 10808, 10809, true)?;
    if after.outbounds.iter().any(|outbound| !before.outbounds.contains(outbound))
        || before.inbounds != after.inbounds
        || before.dns != after.dns
        || before.observatory != after.observatory
        || before.routing["domainStrategy"] != after.routing["domainStrategy"]
        || before.routing["balancers"] != after.routing["balancers"]
    {
        return Ok(None);
    }
    Ok(Some(json!({"routing": after.routing})))
}
pub fn outbound(node: &VpnNode, tag: &str) -> Result<Value> {
    node.validate()?;
    let protocol = match node.protocol {
        Protocol::Vless => "vless",
        Protocol::Vmess => "vmess",
        Protocol::Trojan => "trojan",
        Protocol::Shadowsocks => "shadowsocks",
        Protocol::Hysteria => "hysteria",
        Protocol::Wireguard => "wireguard",
    };
    let settings = match node.protocol {
        Protocol::Wireguard => {
            serde_json::to_value(node.wireguard.as_ref().ok_or_else(BackendError::invalid)?)
                .map_err(|_| BackendError::invalid())?
        }
        Protocol::Hysteria => json!({"version":2,"address":node.address,"port":node.port}),
        Protocol::Vless => {
            json!({"vnext":[{"address":node.address,"port":node.port,"users":[{"id":node.credential,"encryption":"none","flow":node.flow}]}]})
        }
        Protocol::Vmess => {
            json!({"vnext":[{"address":node.address,"port":node.port,"users":[{"id":node.credential,"security":if node.method.is_empty(){"auto"}else{&node.method}}]}]})
        }
        Protocol::Trojan => {
            json!({"servers":[{"address":node.address,"port":node.port,"password":node.credential}]})
        }
        Protocol::Shadowsocks => {
            json!({"servers":[{"address":node.address,"port":node.port,"password":node.credential,"method":node.method}]})
        }
    };
    if node.protocol == Protocol::Wireguard {
        return Ok(json!({"tag":tag,"protocol":protocol,"settings":settings}));
    }
    let t = &node.transport;
    let s = &node.security;
    let mut stream = json!({"network":t.network,"security":s.kind});
    if s.kind == "tls" || s.kind == "reality" {
        let mut security = json!({"serverName":s.server_name,"fingerprint":if s.fingerprint.is_empty(){"chrome"}else{&s.fingerprint}});
        if !s.alpn.is_empty() {
            security["alpn"] = json!(s.alpn);
        }
        if s.kind == "reality" {
            security["password"] = json!(s.public_key);
            security["shortId"] = json!(s.short_id);
            security["spiderX"] = json!(s.spider_x);
        }
        stream[if s.kind == "tls" {
            "tlsSettings"
        } else {
            "realitySettings"
        }] = security;
    }
    match t.network.as_str() {
        "hysteria" => stream["hysteriaSettings"] = json!({"version":2,"auth":node.credential}),
        "kcp" | "mkcp" => {
            stream["kcpSettings"] = json!({"seed":t.seed,"header":{"type":if t.header_type.is_empty(){"none"}else{&t.header_type}}})
        }
        "ws" => stream["wsSettings"] = json!({"path":t.path,"headers":{"Host":t.host}}),
        "grpc" => {
            stream["grpcSettings"] =
                json!({"serviceName":t.service_name,"authority":t.host,"multiMode":t.mode=="multi"})
        }
        "httpupgrade" => stream["httpupgradeSettings"] = json!({"path":t.path,"host":t.host}),
        "xhttp" => {
            stream["xhttpSettings"] = json!({"path":t.path,"host":t.host,"mode":if t.mode.is_empty(){"auto"}else{&t.mode}})
        }
        _ => {}
    }
    Ok(json!({"tag":tag,"protocol":protocol,"settings":settings,"streamSettings":stream}))
}
pub fn build(
    repo: &Repository,
    selected: &str,
    proxy_port: u16,
    metrics_port: u16,
    tun: bool,
) -> Result<XrayConfig> {
    let node = repo
        .nodes
        .iter()
        .find(|n| n.id == selected)
        .ok_or_else(|| BackendError::new("INVALID_SERVER", "Select an available server"))?;

    let (mut outbounds, balancer_tag, observatory) = if let Some(balancer) = &node.balancer {
        let mut obs = balancer.outbounds.clone();
        obs.push(json!({"tag":"direct","protocol":"freedom"}));
        obs.push(json!({"tag":"block","protocol":"blackhole"}));
        (
            obs,
            Some(balancer.tag.clone()),
            balancer.observatory.clone(),
        )
    } else {
        (
            vec![
                outbound(node, "proxy")?,
                json!({"tag":"direct","protocol":"freedom"}),
                json!({"tag":"block","protocol":"blackhole"}),
            ],
            None,
            None,
        )
    };

    let target_tag = balancer_tag.as_deref().unwrap_or("proxy");
    let target_field = if balancer_tag.is_some() {
        "balancerTag"
    } else {
        "outboundTag"
    };

    let mut rules = vec![
        json!({"type":"field","inboundTag":["probe"], target_field: target_tag}),
        json!({"type":"field","process":["self/"],"outboundTag":"direct"}),
    ];
    let dns_servers = if tun || repo.routing_mode == "rules" {
        if repo.settings.dns_server == "system" {
            #[cfg(windows)] { crate::platform::network::system_dns()? }
            #[cfg(not(windows))] { return Err(BackendError::new("PLATFORM_UNSUPPORTED", "System DNS requires Windows")); }
        } else { vec![repo.settings.dns_server.clone()] }
    } else { vec![] };
    // System/private resolvers stay reachable on the physical network. Explicit public
    // resolvers use the selected VPN, including balancers. This tag precedes self/LAN.
    let dns = if tun || repo.routing_mode == "rules" {
        let local = repo.settings.dns_server == "system" || dns_servers.iter().any(|s| {
            s.parse::<std::net::IpAddr>().is_ok_and(|ip| match ip {
                std::net::IpAddr::V4(ip) => ip.is_private() || ip.is_loopback() || ip.is_link_local(),
                std::net::IpAddr::V6(ip) => ip.is_loopback() || ip.is_unique_local() || ip.is_unicast_link_local(),
            })
        });
        if tun || repo.routing_mode == "rules" {
            rules.insert(0, if local {
                json!({"type":"field","inboundTag":["dns-query"],"outboundTag":"direct"})
            } else { json!({"type":"field","inboundTag":["dns-query"],target_field:target_tag}) });
        }
        let servers: Vec<_> = dns_servers.iter().map(|s| if s.contains(':') { format!("tcp://[{s}]") } else { format!("tcp://{s}") }).collect();
        Some(json!({"tag":"dns-query","servers":servers}))
    } else { None };
    // Hijack A/AAAA before LAN bypass, including queries originally sent to a local router.
    if tun || repo.routing_mode == "rules" {
        outbounds.push(json!({"tag":"dns","protocol":"dns"}));
    }
    if repo.routing_mode == "rules" {
        rules.push(json!({"type":"field","port":"53","outboundTag":"dns"}));
    }
    if repo.settings.bypass_lan {
        rules.push(json!({"type":"field","ip":["127.0.0.0/8","10.0.0.0/8","172.16.0.0/12","192.168.0.0/16","169.254.0.0/16","::1/128","fc00::/7","fe80::/10"],"outboundTag":"direct"}));
    }
    for rule in crate::services::catalog_service::ordered_rules(&repo.rules) {
            let target = if ["direct", "block"].contains(&rule.server_id.as_str()) {
                rule.server_id.clone()
            } else if rule.server_id == "subscription"
                || rule.server_id == selected
                || rule.server_id.is_empty()
            {
                target_tag.to_string()
            } else if let Some(target) =
                repo.nodes
                    .iter()
                    .find(|n| n.id == rule.server_id)
                    .or_else(|| {
                        repo.nodes
                            .iter()
                            .find(|n| n.subscription_id == rule.server_id)
                    })
            {
                if target.balancer.is_none() && !outbounds.iter().any(|o| o["tag"] == target.id) {
                    if let Ok(ob) = outbound(target, &target.id) {
                        outbounds.push(ob);
                    }
                }
                target.id.clone()
            } else {
                target_tag.to_string()
            };
            // Keep an outbound available when its rule is temporarily disabled,
            // so re-enabling it only replaces routing rules through the live API.
            if !rule.enabled || repo.routing_mode != "rules" { continue; }
            let is_balancer_target = balancer_tag.as_deref() == Some(&target);
            let mut compiled = if is_balancer_target {
                json!({"type":"field","balancerTag":target})
            } else {
                json!({"type":"field","outboundTag":target})
            };
            if rule.name.is_empty()
                || rule.name.len() > 1024
                || rule.name.chars().any(char::is_control)
            {
                return Err(BackendError::new("INVALID_RULE", "Invalid routing rule"));
            }
            match rule.matcher {
                crate::storage::repository::RuleMatcher::Domain => {
                    let value = rule.name.trim();
                    if let Some((address, prefix)) = value.split_once('/') {
                        if let Ok(ip) = address.parse::<std::net::IpAddr>() {
                            let prefix: u8 = prefix.parse().map_err(|_| {
                                BackendError::new("INVALID_RULE", "Invalid network prefix")
                            })?;
                            if prefix > if ip.is_ipv4() { 32 } else { 128 } {
                                return Err(BackendError::new(
                                    "INVALID_RULE",
                                    "Invalid network prefix",
                                ));
                            }
                            compiled["ip"] = json!([value]);
                        } else {
                            compiled["domain"] = json!([domain_rule(value)?]);
                        }
                    } else if value.parse::<std::net::IpAddr>().is_ok() {
                        compiled["ip"] = json!([value]);
                    } else {
                        compiled["domain"] = json!([domain_rule(value)?]);
                    }
                }
                crate::storage::repository::RuleMatcher::Application => {
                    compiled["process"] = json!(rule
                        .name
                        .split(',')
                        .map(|p| p.trim().replace('\\', "/"))
                        .collect::<Vec<_>>());
                }
                crate::storage::repository::RuleMatcher::Geosite => {
                    let tag = crate::storage::repository::normalize_geo_tag(&rule.name, "geosite")?;
                    compiled["domain"] = json!([format!("geosite:{tag}")]);
                }
                crate::storage::repository::RuleMatcher::Geoip => {
                    let tag = crate::storage::repository::normalize_geo_tag(&rule.name, "geoip")?;
                    compiled["ip"] = json!([format!("geoip:{tag}")]);
                }
            }
            rules.push(compiled);
    }
    let default_field = if repo.routing_mode == "rules" || balancer_tag.is_none() {
        "outboundTag"
    } else {
        "balancerTag"
    };
    let default_val = if repo.routing_mode == "rules" {
        "direct"
    } else {
        target_tag
    };
    rules.push(json!({"type":"field","network":"tcp,udp", default_field: default_val}));
    let mut inbounds = vec![
        json!({"tag":"probe","listen":"127.0.0.1","port":proxy_port,"protocol":"socks","settings":{"auth":"noauth","udp":true}}),
    ];
    if !tun {
        inbounds.push(json!({"tag":"system-proxy","listen":"127.0.0.1","port":metrics_port.checked_add(1).ok_or_else(BackendError::invalid)?,"protocol":"http","settings":{}}));
    }
    if tun {
        inbounds.push(json!({"tag":"tun","protocol":"tun","settings":{"name":"LumenTun","mtu":1500,"gateway":["172.31.255.1/30","fd42:6c75:6d65::1/126"],"dns":dns_servers,"autoSystemRoutingTable":["0.0.0.0/0","::/0"],"autoOutboundsInterface":"auto"},"sniffing":{"enabled":true,"destOverride":["http","tls","quic"],"routeOnly":true}}));
    }

    // TUN keeps one router strategy in both modes so its rules can change
    // without tearing down the adapter. The global catch-all still selects VPN.
    let mut routing = json!({"domainStrategy":if tun || repo.routing_mode == "rules" { "IPOnDemand" } else { "IPIfNonMatch" },"rules":rules});
    if let Some(balancer) = &node.balancer {
        routing["balancers"] = json!([{
            "tag": balancer.tag,
            "selector": balancer.selector,
            "strategy": balancer.strategy
        }]);
    }

    Ok(XrayConfig {
        log: json!({"loglevel":"warning"}),
        api: None,
        inbounds,
        outbounds,
        routing,
        dns,
        stats: json!({}),
        policy: json!({"system":{"statsOutboundUplink":true,"statsOutboundDownlink":true}}),
        metrics: json!({"tag":"metrics","listen":format!("127.0.0.1:{metrics_port}")}),
        observatory,
    })
}
fn domain_rule(value: &str) -> Result<String> {
    let value = value.trim_end_matches('/');
    let value = value.strip_prefix("*.").unwrap_or(value);
    let value = value.to_lowercase();
    if value.is_empty() || value.contains(['/', ':', ' ']) {
        return Ok("domain:localhost".to_string());
    }
    match url::Host::parse(&value) {
        Ok(host) => Ok(format!("domain:{host}")),
        Err(_) => {
            let cleaned: String = value
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '.' || *c == '-')
                .collect();
            if cleaned.is_empty() {
                Ok("domain:localhost".to_string())
            } else {
                Ok(format!("domain:{cleaned}"))
            }
        }
    }
}
