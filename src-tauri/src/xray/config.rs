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
    inbounds: Vec<Value>,
    outbounds: Vec<Value>,
    routing: Value,
    stats: Value,
    policy: Value,
    metrics: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    observatory: Option<Value>,
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
    if repo.settings.bypass_lan {
        rules.push(json!({"type":"field","ip":["127.0.0.0/8","10.0.0.0/8","172.16.0.0/12","192.168.0.0/16","169.254.0.0/16","::1/128","fc00::/7","fe80::/10"],"outboundTag":"direct"}));
    }
    if repo.routing_mode == "rules" {
        for rule in repo.rules.iter().filter(|r| r.enabled) {
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

        // In rules mode, route unhandled DNS queries (port 53) through the VPN proxy so domain resolution doesn't fail
        rules.push(json!({"type":"field","port":"53", target_field: target_tag}));
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
        let dns = if repo.settings.dns_server == "system" {
            #[cfg(windows)]
            {
                crate::platform::network::system_dns()?
            }
            #[cfg(not(windows))]
            {
                return Err(BackendError::new(
                    "PLATFORM_UNSUPPORTED",
                    "System DNS requires Windows",
                ));
            }
        } else {
            vec![repo.settings.dns_server.clone()]
        };
        inbounds.push(json!({"tag":"tun","protocol":"tun","settings":{"name":"LumenTun","mtu":1500,"gateway":["172.31.255.1/30","fd42:6c75:6d65::1/126"],"dns":dns,"autoSystemRoutingTable":["0.0.0.0/0","::/0"],"autoOutboundsInterface":"auto"},"sniffing":{"enabled":true,"destOverride":["http","tls","quic"],"routeOnly":true}}));
    }

    // Domain Resolution Strategy:
    // We use "IPIfNonMatch" according to official Xray routing documentation:
    // 1. "AsIs" would fail to evaluate GeoIP rules for domain/sniffed traffic because domains
    //    are never resolved to IPs during rule evaluation.
    // 2. "IPOnDemand" triggers immediate DNS resolution as soon as the first IP rule is reached,
    //    causing unnecessary latency even when subsequent domain/geosite rules would have matched.
    // 3. "IPIfNonMatch" first tests all domain and geosite rules against the SNI/domain (fast path).
    //    If none match, Xray resolves the destination domain to an IP and tests GeoIP/IP rules (second pass).
    // 4. In TUN mode, TLS/HTTP/QUIC sniffing with `routeOnly: true` provides the domain for matching,
    //    while unhandled port 53 DNS queries are routed via the proxy outbound, preventing DNS leaks or breaks.
    let mut routing = json!({"domainStrategy":"IPIfNonMatch","rules":rules});
    if let Some(balancer) = &node.balancer {
        routing["balancers"] = json!([{
            "tag": balancer.tag,
            "selector": balancer.selector,
            "strategy": balancer.strategy
        }]);
    }

    Ok(XrayConfig {
        log: json!({"loglevel":"warning"}),
        inbounds,
        outbounds,
        routing,
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
