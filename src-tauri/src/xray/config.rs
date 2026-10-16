use crate::{
    domain::{
        errors::{BackendError, Result},
        node::{NodeIndex, Protocol, VpnNode},
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
    let tun = config
        .inbounds
        .iter_mut()
        .find(|inbound| inbound["protocol"] == "tun")
        .ok_or_else(BackendError::invalid)?;
    tun["settings"]
        .as_object_mut()
        .ok_or_else(BackendError::invalid)?
        .remove("autoSystemRoutingTable");
    enable_api(&mut config, metrics_port)?;
    Ok(config)
}

/// Local-only control API used to replace routing and add outbounds without a restart.
pub fn enable_api(config: &mut XrayConfig, metrics_port: u16) -> Result<()> {
    let api_port = metrics_port
        .checked_add(2)
        .ok_or_else(BackendError::invalid)?;
    config.api = Some(
        json!({"tag":"lumen-api","listen":format!("127.0.0.1:{api_port}"),"services":["RoutingService","HandlerService"]}),
    );
    Ok(())
}

/// Routing replacement plus node outbounds that the running core does not have yet.
#[cfg(windows)]
pub struct LiveRoutingPlan {
    pub patch: Value,
    pub added_outbounds: Vec<Value>,
}

/// Xray replaces the complete rule set atomically through RoutingService.
/// Inbounds, DNS, and the router strategy must stay identical. A rule that
/// points at a server which is not loaded yet adds that outbound first.
#[cfg(windows)]
pub fn live_routing_plan(
    repo_before: &Repository,
    repo_after: &Repository,
    selected: &str,
    tun: bool,
) -> Result<Option<LiveRoutingPlan>> {
    let before = build(repo_before, selected, 10808, 10809, tun)?;
    let after = build(repo_after, selected, 10808, 10809, tun)?;
    if before.inbounds != after.inbounds
        || before.dns != after.dns
        || before.observatory != after.observatory
        || before.routing["domainStrategy"] != after.routing["domainStrategy"]
        || before.routing["balancers"] != after.routing["balancers"]
    {
        return Ok(None);
    }
    let mut added_outbounds = Vec::new();
    for outbound in &after.outbounds {
        if before.outbounds.contains(outbound) {
            continue;
        }
        let tag = outbound["tag"].as_str().unwrap_or("");
        // The same tag with a new body is a credential or transport change.
        // Built-in tags are part of the process, not a rule target.
        if tag.is_empty()
            || matches!(tag, "proxy" | "direct" | "block" | "dns")
            || before
                .outbounds
                .iter()
                .any(|existing| existing["tag"].as_str() == Some(tag))
        {
            return Ok(None);
        }
        added_outbounds.push(outbound.clone());
    }
    Ok(Some(LiveRoutingPlan {
        patch: json!({"routing": after.routing}),
        added_outbounds,
    }))
}

/// Patch-only view of `live_routing_plan`, used by tests and the debug smoke run.
#[cfg(all(windows, any(test, debug_assertions)))]
pub fn live_routing_patch(
    repo_before: &Repository,
    repo_after: &Repository,
    selected: &str,
    tun: bool,
) -> Result<Option<Value>> {
    Ok(live_routing_plan(repo_before, repo_after, selected, tun)?.map(|plan| plan.patch))
}
pub fn outbound(node: &VpnNode, tag: &str) -> Result<Value> {
    node.validate()?;
    let protocol = match node.protocol {
        Protocol::Socks => "socks",
        Protocol::Http => "http",
        Protocol::Vless => "vless",
        Protocol::Vmess => "vmess",
        Protocol::Trojan => "trojan",
        Protocol::Shadowsocks => "shadowsocks",
        Protocol::Hysteria => "hysteria",
        Protocol::Wireguard => "wireguard",
    };
    let settings = match node.protocol {
        Protocol::Socks | Protocol::Http => {
            let mut server = json!({"address":node.address,"port":node.port});
            if !node.method.is_empty() {
                server["users"] = json!([{"user":node.method,"pass":node.credential}]);
            }
            json!({"servers":[server]})
        }
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
    // DNS, inbounds, outbounds and the router strategy must not depend on the
    // routing mode: switching All/Rules then only replaces rules through the API
    // in both TUN and system-proxy modes, without restarting the core.
    let dns_servers = if repo.settings.dns_policy.enabled {
        vec![repo.settings.dns_policy.bootstrap.clone()]
    } else if repo.settings.dns_server == "system" {
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
    // System/private resolvers stay reachable on the physical network. Explicit public
    // resolvers use the selected VPN, including balancers. This tag precedes self/LAN.
    let dns = if repo.settings.dns_policy.enabled {
        Some(super::dns_policy::build(
            repo,
            target_field,
            target_tag,
            &mut rules,
        )?)
    } else {
        let local = repo.settings.dns_server == "system"
            || dns_servers.iter().any(|s| {
                s.parse::<std::net::IpAddr>().is_ok_and(|ip| match ip {
                    std::net::IpAddr::V4(ip) => {
                        ip.is_private() || ip.is_loopback() || ip.is_link_local()
                    }
                    std::net::IpAddr::V6(ip) => {
                        ip.is_loopback() || ip.is_unique_local() || ip.is_unicast_link_local()
                    }
                })
            });
        // In Rules mode the resolver is reached directly. Through the VPN, one stalled
        // server took every lookup with it, so direct sites went down as well. The
        // sniffed domain still goes to the VPN for proxied traffic, so a poisoned
        // direct answer does not decide where a blocked site connects.
        rules.insert(
            0,
            if local || repo.routing_mode == "rules" {
                json!({"type":"field","inboundTag":["dns-query"],"outboundTag":"direct"})
            } else {
                json!({"type":"field","inboundTag":["dns-query"],target_field:target_tag})
            },
        );
        // A local resolver is always reached directly, so it keeps plain UDP: many
        // home routers do not answer DNS over TCP. Public resolvers keep TCP, which
        // also survives the VPN path in global mode.
        let servers: Vec<_> = dns_servers
            .iter()
            .map(|s| {
                if local {
                    s.clone()
                } else if s.contains(':') {
                    format!("tcp://[{s}]")
                } else {
                    format!("tcp://{s}")
                }
            })
            .collect();
        Some(json!({"tag":"dns-query","servers":servers}))
    };
    // Hijack A/AAAA before LAN bypass, including queries originally sent to a local router.
    outbounds.push(json!({"tag":"dns","protocol":"dns"}));
    if repo.routing_mode == "rules" || (tun && repo.settings.dns_policy.enabled) {
        rules.push(json!({"type":"field","port":"53","outboundTag":"dns"}));
    }
    if repo.settings.bypass_lan {
        rules.push(json!({"type":"field","ip":["127.0.0.0/8","10.0.0.0/8","172.16.0.0/12","192.168.0.0/16","169.254.0.0/16","::1/128","fc00::/7","fe80::/10"],"outboundTag":"direct"}));
    }
    let mut nodes = None;
    let mut outbound_tags: Option<std::collections::HashSet<String>> = None;
    for rule in crate::services::catalog_service::ordered_rules(&repo.rules) {
        let target = if ["direct", "block"].contains(&rule.server_id.as_str()) {
            rule.server_id.clone()
        } else if rule.server_id == "subscription"
            || rule.server_id == selected
            || rule.server_id.is_empty()
        {
            target_tag.to_string()
        } else if let Some(target) = nodes
            .get_or_insert_with(|| NodeIndex::new(&repo.nodes))
            .target(&rule.server_id)
        {
            if target.balancer.is_none() {
                let tags = outbound_tags.get_or_insert_with(|| {
                    outbounds
                        .iter()
                        .filter_map(|outbound| outbound["tag"].as_str().map(str::to_owned))
                        .collect()
                });
                if !tags.contains(&target.id) {
                    if let Ok(ob) = outbound(target, &target.id) {
                        tags.insert(target.id.clone());
                        outbounds.push(ob);
                    }
                }
            }
            target.id.clone()
        } else {
            target_tag.to_string()
        };
        // Keep an outbound available when its rule is temporarily disabled,
        // so re-enabling it only replaces routing rules through the live API.
        if !rule.enabled || repo.routing_mode != "rules" {
            continue;
        }
        if rule.name.is_empty() || rule.name.len() > 1024 || rule.name.chars().any(char::is_control)
        {
            return Err(BackendError::new("INVALID_RULE", "Invalid routing rule"));
        }
        let is_balancer_target = balancer_tag.as_deref() == Some(target.as_str());
        let mut compiled = if is_balancer_target {
            json!({"type":"field","balancerTag":target.clone()})
        } else {
            json!({"type":"field","outboundTag":target.clone()})
        };
        match rule.matcher {
            crate::storage::repository::RuleMatcher::Domain => {
                let value = rule.name.trim();
                if let Some(keyword) = value.strip_prefix("keyword:") {
                    if keyword.is_empty()
                        || keyword.len() > 128
                        || keyword.chars().any(|c| {
                            !(c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
                        })
                    {
                        return Err(BackendError::new("INVALID_RULE", "Invalid routing rule"));
                    }
                    compiled["domain"] = json!([format!("keyword:{keyword}")]);
                } else if let Some(host) = value.strip_prefix("full:") {
                    let exact = domain_rule(host)?;
                    let host = exact.strip_prefix("domain:").unwrap_or(host);
                    compiled["domain"] = json!([format!("full:{host}")]);
                } else if let Some((address, prefix)) = value.split_once('/') {
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
                compiled["process"] = json!(process_names(&rule.name));
            }
            crate::storage::repository::RuleMatcher::Geosite => {
                let tag = crate::storage::repository::normalize_geo_tag(&rule.name, "geosite")?;
                compiled["domain"] = json!([format!("geosite:{tag}")]);
            }
            crate::storage::repository::RuleMatcher::Geoip => {
                let tag = crate::storage::repository::normalize_geo_tag(&rule.name, "geoip")?;
                compiled["ip"] = json!([format!("geoip:{tag}")]);
            }
            // A MATCH/final rule must not turn Rules mode into a full tunnel.
            // Unmatched traffic stays direct; only the listed services use the VPN.
            crate::storage::repository::RuleMatcher::Final => continue,
        }
        rules.push(compiled);
    }
    let default_target = if repo.routing_mode == "rules" {
        "direct".to_string()
    } else {
        target_tag.to_string()
    };
    let default_field = if balancer_tag.as_deref() == Some(default_target.as_str()) {
        "balancerTag"
    } else {
        "outboundTag"
    };
    if repo.routing_mode == "rules" {
        // IPIfNonMatch resolves a domain only when no rule matched it. A network
        // catch-all always matched, so GeoIP, antifilter and Telegram ranges and
        // manual addresses never saw a domain. An address catch-all leaves room for
        // that second pass. A name the direct resolver cannot answer falls through
        // to the first outbound (the VPN), which resolves it on the server.
        rules
            .push(json!({"type":"field","ip":["0.0.0.0/0","::/0"], default_field: default_target}));
    } else {
        rules.push(json!({"type":"field","network":"tcp,udp", default_field: default_target}));
    }
    if repo.settings.dns_policy.enabled {
        for outbound in &mut outbounds {
            if matches!(
                outbound["protocol"].as_str(),
                Some("vless" | "vmess" | "trojan" | "shadowsocks" | "hysteria")
            ) {
                if outbound.get("streamSettings").is_none() {
                    outbound["streamSettings"] = json!({});
                }
                if outbound["streamSettings"].get("sockopt").is_none() {
                    outbound["streamSettings"]["sockopt"] = json!({});
                }
                outbound["streamSettings"]["sockopt"]["domainStrategy"] = json!("UseIP");
            }
        }
    }
    let mut inbounds = vec![
        json!({"tag":"probe","listen":"127.0.0.1","port":proxy_port,"protocol":"socks","settings":{"auth":"noauth","udp":true}}),
    ];
    if !tun {
        inbounds.push(json!({"tag":"system-proxy","listen":"127.0.0.1","port":metrics_port.checked_add(1).ok_or_else(BackendError::invalid)?,"protocol":"http","settings":{}}));
    }
    if tun {
        // Capture IPv6 only when the physical network has it; see physical_ipv6_available.
        #[cfg(windows)]
        let routes = if crate::platform::network::physical_ipv6_available() {
            json!(["0.0.0.0/0", "::/0"])
        } else {
            json!(["0.0.0.0/0"])
        };
        #[cfg(not(windows))]
        let routes = json!(["0.0.0.0/0", "::/0"]);
        let ipv6 = routes.as_array().is_some_and(|routes| routes.len() > 1);
        // The sniffed domain replaces the destination address. The VPN server then
        // resolves blocked sites itself, so a stub or foreign-CDN answer from the
        // direct resolver cannot send them to a dead address. Direct traffic resolves
        // through Xray's own DNS (cached, direct in Rules mode) instead of looping the
        // Windows resolver back through the adapter.
        inbounds.push(json!({"tag":"tun","protocol":"tun","settings":{"name":"LumenTun","mtu":1500,"gateway":["172.31.255.1/30","fd42:6c75:6d65::1/126"],"dns":dns_servers,"autoSystemRoutingTable":routes,"autoOutboundsInterface":"auto"},"sniffing":{"enabled":true,"destOverride":["http","tls","quic"],"routeOnly":false,"domainsExcluded":[
            // Local names resolve through LLMNR/mDNS, not Xray's resolver: keep the
            // address so the LAN bypass applies.
            "regexp:^[^.]+$","regexp:(^|\\.)(local|lan|home\\.arpa|internal)$","courier.push.apple.com"]}}));
        for outbound in &mut outbounds {
            if outbound["tag"] == "direct" && outbound["protocol"] == "freedom" {
                outbound["settings"] =
                    json!({"domainStrategy": if ipv6 { "UseIPv4v6" } else { "UseIPv4" }});
            }
        }
    }

    // One router strategy in both routing modes lets rules change without
    // restarting the core. The global catch-all still selects the VPN.
    // Match the domain first. IPOnDemand resolved every site before the Russian
    // domain rule, so a .ru site on a foreign CDN was sent through the VPN.
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
/// Xray compares a process name case-sensitively with the executable on disk,
/// while Windows names are not: an `example.exe` rule must also match `Example.exe`.
/// Emit the spellings Windows applications use. Paths are matched as written.
pub(crate) fn process_names(value: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for entry in value.split(',') {
        let name = entry.trim().replace('\\', "/");
        if name.is_empty() {
            continue;
        }
        let mut variants = vec![name.clone()];
        if !name.contains('/') {
            let lower = name.to_lowercase();
            let mut chars = lower.chars();
            let title = chars
                .next()
                .map(|first| first.to_uppercase().chain(chars).collect::<String>())
                .unwrap_or_default();
            variants.extend([lower, title, name.to_uppercase()]);
        }
        for variant in variants {
            if !names.contains(&variant) {
                names.push(variant);
            }
        }
    }
    names
}
pub(crate) fn domain_rule(value: &str) -> Result<String> {
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
