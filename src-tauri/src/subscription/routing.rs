//! Routing shipped with a subscription.
//!
//! Clash profiles carry a `rules` list. Incy profiles arrive as a base64 JSON
//! document in the `routing` response header or as an `incy://routing/onadd/…`
//! line in the body. Both become ordinary rules owned by that subscription.

use crate::storage::repository::{normalize_geo_tag, Rule, RuleMatcher};
use base64::Engine;
use serde_json::Value;

const MAX_RULES: usize = 512;
const MAX_PROFILE_BYTES: usize = 256 * 1024;

pub enum LineRouting {
    Off,
    Profile(Value),
}

pub enum HeaderRouting {
    Off,
    Profile(Value),
    Absent,
}

pub fn subscription_preset_id(subscription: &str) -> String {
    format!("subscription:{subscription}")
}

pub fn line_routing(line: &str) -> Option<LineRouting> {
    let line = line.trim();
    if is_off(line) {
        return Some(LineRouting::Off);
    }
    let payload = routing_payload(line)?;
    decode_profile(payload).map(LineRouting::Profile)
}

pub fn header_routing(value: &str) -> HeaderRouting {
    let value = value.trim();
    if value.is_empty() {
        return HeaderRouting::Absent;
    }
    if is_off(value) {
        return HeaderRouting::Off;
    }
    if let Some(payload) = routing_payload(value) {
        return decode_profile(payload)
            .map(HeaderRouting::Profile)
            .unwrap_or(HeaderRouting::Absent);
    }
    decode_profile(value)
        .map(HeaderRouting::Profile)
        .unwrap_or(HeaderRouting::Absent)
}

pub fn from_clash(
    rules: &Value,
    subscription: &str,
    nodes: &[crate::domain::node::VpnNode],
) -> Vec<Rule> {
    let Some(list) = rules.as_array() else {
        return Vec::new();
    };
    let preset = subscription_preset_id(subscription);
    let mut parsed = Vec::new();
    for rule in list {
        if parsed.len() >= MAX_RULES {
            break;
        }
        let Some(text) = rule.as_str() else {
            continue;
        };
        push_clash_rule(&mut parsed, &preset, text, nodes);
    }
    parsed
}

pub fn from_profiles(
    profiles: &[Value],
    subscription: &str,
    nodes: &[crate::domain::node::VpnNode],
) -> Vec<Rule> {
    let preset = subscription_preset_id(subscription);
    let mut rules = Vec::new();
    let mut fallback: Option<bool> = None;
    for profile in profiles {
        if !profile.is_object() {
            continue;
        }
        push_incy_list(
            &mut rules,
            &preset,
            &profile["BlockSites"],
            "block",
            nodes,
            false,
        );
        push_incy_list(
            &mut rules,
            &preset,
            &profile["BlockIp"],
            "block",
            nodes,
            true,
        );
        push_incy_list(
            &mut rules,
            &preset,
            &profile["DirectSites"],
            "direct",
            nodes,
            false,
        );
        push_incy_list(
            &mut rules,
            &preset,
            &profile["DirectIp"],
            "direct",
            nodes,
            true,
        );
        push_incy_list(
            &mut rules,
            &preset,
            &profile["ProxySites"],
            "subscription",
            nodes,
            false,
        );
        push_incy_list(
            &mut rules,
            &preset,
            &profile["ProxyIp"],
            "subscription",
            nodes,
            true,
        );
        fallback = Some(global_proxy(profile));
    }
    if let Some(through_proxy) = fallback {
        push(
            &mut rules,
            &preset,
            RuleMatcher::Final,
            "match",
            if through_proxy {
                "subscription"
            } else {
                "direct"
            },
            nodes,
        );
    }
    rules
}

fn push_clash_rule(
    rules: &mut Vec<Rule>,
    preset: &str,
    text: &str,
    nodes: &[crate::domain::node::VpnNode],
) {
    let fields: Vec<&str> = text.split(',').map(str::trim).collect();
    if fields.len() < 2 {
        return;
    }
    let kind = fields[0].to_ascii_uppercase();
    let payload = fields[1];
    let policy = if kind == "MATCH" {
        payload
    } else if fields.len() >= 3 {
        fields[2]
    } else {
        return;
    };
    if policy.eq_ignore_ascii_case("no-resolve") {
        return;
    }
    match kind.as_str() {
        "MATCH" => push(rules, preset, RuleMatcher::Final, "match", policy, nodes),
        "PROCESS-NAME" if payload == "*" => {
            push(rules, preset, RuleMatcher::Final, "match", policy, nodes);
        }
        "PROCESS-NAME" if valid_process(payload) => {
            push(
                rules,
                preset,
                RuleMatcher::Application,
                payload,
                policy,
                nodes,
            );
        }
        "GEOSITE" => {
            if let Ok(tag) = normalize_geo_tag(&payload.to_ascii_lowercase(), "geosite") {
                push(rules, preset, RuleMatcher::Geosite, &tag, policy, nodes);
            }
        }
        "GEOIP" => {
            if let Ok(tag) = normalize_geo_tag(&payload.to_ascii_lowercase(), "geoip") {
                push(rules, preset, RuleMatcher::Geoip, &tag, policy, nodes);
            }
        }
        "DOMAIN-SUFFIX" | "DOMAIN" if valid_host(payload) => {
            let name = if kind == "DOMAIN" {
                format!("full:{}", payload.to_ascii_lowercase())
            } else {
                payload.to_ascii_lowercase()
            };
            push(rules, preset, RuleMatcher::Domain, &name, policy, nodes);
        }
        "DOMAIN-KEYWORD" if valid_keyword(payload) => {
            push(
                rules,
                preset,
                RuleMatcher::Domain,
                &format!("keyword:{}", payload.to_ascii_lowercase()),
                policy,
                nodes,
            );
        }
        "IP-CIDR" | "IP-CIDR6" if valid_network(payload) => {
            push(rules, preset, RuleMatcher::Domain, payload, policy, nodes);
        }
        _ => {}
    }
}

fn push_incy_list(
    rules: &mut Vec<Rule>,
    preset: &str,
    value: &Value,
    server: &str,
    nodes: &[crate::domain::node::VpnNode],
    addresses: bool,
) {
    let Some(list) = value.as_array() else {
        return;
    };
    for item in list {
        let Some(item) = item.as_str() else {
            continue;
        };
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        let lower = item.to_ascii_lowercase();
        if let Some(tag) = lower.strip_prefix("geosite:") {
            if let Ok(tag) = normalize_geo_tag(tag, "geosite") {
                push(rules, preset, RuleMatcher::Geosite, &tag, server, nodes);
            }
        } else if let Some(tag) = lower.strip_prefix("geoip:") {
            if let Ok(tag) = normalize_geo_tag(tag, "geoip") {
                push(rules, preset, RuleMatcher::Geoip, &tag, server, nodes);
            }
        } else if let Some(keyword) = lower.strip_prefix("keyword:") {
            if valid_keyword(keyword) {
                push(
                    rules,
                    preset,
                    RuleMatcher::Domain,
                    &format!("keyword:{keyword}"),
                    server,
                    nodes,
                );
            }
        } else if lower.strip_prefix("regexp:").is_some() {
            // Xray regexp rules are not accepted from a subscription.
        } else if addresses && valid_network(item) {
            push(rules, preset, RuleMatcher::Domain, item, server, nodes);
        } else if !addresses {
            let host = lower
                .strip_prefix("full:")
                .map(|host| format!("full:{host}"))
                .unwrap_or(lower.strip_prefix("domain:").unwrap_or(&lower).to_string());
            let bare = host.strip_prefix("full:").unwrap_or(&host);
            if valid_host(bare) {
                push(rules, preset, RuleMatcher::Domain, &host, server, nodes);
            }
        }
    }
}

fn push(
    rules: &mut Vec<Rule>,
    preset: &str,
    matcher: RuleMatcher,
    name: &str,
    policy: &str,
    nodes: &[crate::domain::node::VpnNode],
) {
    if rules.len() >= MAX_RULES || name.is_empty() || name.len() > 1024 {
        return;
    }
    let server_id = policy_server(policy, nodes);
    let index = rules.len();
    rules.push(Rule {
        id: format!("{preset}-{index}"),
        name: name.to_string(),
        matcher,
        server_id,
        enabled: true,
        preset_id: Some(preset.to_string()),
        is_domain: matcher == RuleMatcher::Domain,
    });
}

fn policy_server(policy: &str, nodes: &[crate::domain::node::VpnNode]) -> String {
    let policy = policy.trim();
    match policy.to_ascii_uppercase().as_str() {
        "DIRECT" => return "direct".into(),
        "REJECT" | "REJECT-DROP" | "REJECT-TINYGIF" | "DROP" | "BLOCK" => return "block".into(),
        "SUBSCRIPTION" => return "subscription".into(),
        _ => {}
    }
    nodes
        .iter()
        .find(|node| node.name.eq_ignore_ascii_case(policy))
        .map(|node| node.id.clone())
        .unwrap_or_else(|| "subscription".into())
}

fn global_proxy(profile: &Value) -> bool {
    match &profile["GlobalProxy"] {
        Value::Bool(value) => *value,
        Value::String(value) => !value.eq_ignore_ascii_case("false"),
        Value::Null => true,
        _ => true,
    }
}

fn is_off(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "off" | "incy://routing/off" | "://routing/off"
    )
}

fn routing_payload(value: &str) -> Option<&str> {
    let lower = value.to_ascii_lowercase();
    for marker in ["/routing/onadd/", "/routing/add/"] {
        if let Some(index) = lower.find(marker) {
            let payload = value[index + marker.len()..].trim();
            if !payload.is_empty() {
                return Some(payload);
            }
        }
    }
    None
}

fn decode_profile(payload: &str) -> Option<Value> {
    let payload = payload.trim().trim_matches(['"', '\'']);
    let bytes = if payload.starts_with('{') {
        if payload.len() > MAX_PROFILE_BYTES {
            return None;
        }
        payload.as_bytes().to_vec()
    } else {
        decode_base64(payload)?
    };
    if bytes.len() > MAX_PROFILE_BYTES {
        return None;
    }
    let text = String::from_utf8(bytes).ok()?;
    let value: Value = serde_json::from_str(&text).ok()?;
    value.as_object()?;
    Some(value)
}

fn decode_base64(payload: &str) -> Option<Vec<u8>> {
    if payload.len() > MAX_PROFILE_BYTES * 2 {
        return None;
    }
    base64::engine::general_purpose::STANDARD
        .decode(payload)
        .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(payload))
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(payload))
        .or_else(|_| base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload))
        .ok()
}

fn valid_host(value: &str) -> bool {
    let value = value.trim().trim_end_matches('.');
    !value.is_empty()
        && value.len() <= 253
        && !value.contains(['/', ':', ' ', '*'])
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
        && value.contains('.')
}

fn valid_keyword(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
}

fn valid_process(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 260
        && !value.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|'])
        && value.chars().all(|c| !c.is_control())
}

fn valid_network(value: &str) -> bool {
    let Some((address, prefix)) = value.split_once('/') else {
        return value.parse::<std::net::IpAddr>().is_ok();
    };
    let Ok(ip) = address.parse::<std::net::IpAddr>() else {
        return false;
    };
    let Ok(prefix) = prefix.parse::<u8>() else {
        return false;
    };
    prefix <= if ip.is_ipv4() { 32 } else { 128 }
}
