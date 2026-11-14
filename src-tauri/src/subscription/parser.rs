use super::uri;
use crate::domain::{
    errors::{BackendError, Result},
    node::VpnNode,
};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

pub const MAX_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_NODES: usize = 4096;
#[derive(Serialize)]
pub struct ImportDiagnostic {
    pub index: usize,
    pub code: &'static str,
}
pub struct ParsedSubscription {
    pub nodes: Vec<VpnNode>,
    pub diagnostics: Vec<ImportDiagnostic>,
    pub rules: Vec<crate::storage::repository::Rule>,
    /// The provider explicitly turned routing profiles off.
    pub routing_off: bool,
}

pub fn parse(input: &str, subscription: &str) -> Result<ParsedSubscription> {
    if input.len() > MAX_BYTES {
        return Err(BackendError::invalid());
    }
    let input = input.trim().trim_start_matches('\u{feff}');
    let decoded;
    let text = if !input.contains("://")
        && !input.starts_with(['{', '['])
        && !input.contains("proxies:")
    {
        decoded = uri::decode(input)?;
        decoded.trim()
    } else {
        input
    };
    let mut clash_rules: Option<serde_json::Value> = None;
    let mut incy_profiles: Vec<serde_json::Value> = Vec::new();
    let mut routing_off = false;
    let entries: Vec<Result<VpnNode>> = if text.starts_with(['{', '[']) {
        let json: serde_json::Value =
            serde_json::from_str(text).map_err(|_| BackendError::invalid())?;
        let roots = match json {
            serde_json::Value::Array(arr) => arr,
            other => vec![other],
        };
        let mut parsed_entries = Vec::new();
        for root in roots {
            if parsed_entries.len() >= MAX_NODES {
                break;
            }
            if let Some(outbounds) = root.get("outbounds").and_then(|v| v.as_array()) {
                let remarks = root.get("remarks").and_then(|r| r.as_str());
                let mut proxy_outbounds: Vec<&serde_json::Value> = outbounds
                    .iter()
                    .filter(|o| {
                        ![Some("freedom"), Some("blackhole"), Some("dns")]
                            .contains(&o["protocol"].as_str())
                    })
                    .collect();
                // Panels put a dummy loopback outbound next to real servers. A balancer
                // selector also matches it by prefix and sends traffic nowhere.
                if proxy_outbounds.iter().any(|o| !is_placeholder(o)) {
                    proxy_outbounds.retain(|o| !is_placeholder(o));
                }

                if proxy_outbounds.is_empty() {
                    continue;
                }

                let has_balancers = root
                    .get("routing")
                    .and_then(|r| r.get("balancers"))
                    .and_then(|b| b.as_array())
                    .is_some_and(|b| !b.is_empty());

                if has_balancers || proxy_outbounds.len() > 1 {
                    // Auto / Balanced node representing a cluster of servers
                    let (b_tag, selector, strategy) = if let Some(b) = root
                        .get("routing")
                        .and_then(|r| r.get("balancers"))
                        .and_then(|b| b.as_array())
                        .and_then(|a| a.first())
                    {
                        (
                            b["tag"].as_str().unwrap_or("auto").to_string(),
                            b["selector"]
                                .as_array()
                                .map(|arr| {
                                    arr.iter()
                                        .filter_map(|s| s.as_str().map(String::from))
                                        .collect()
                                })
                                .unwrap_or_else(|| vec!["proxy".to_string()]),
                            b.get("strategy")
                                .cloned()
                                .unwrap_or_else(|| serde_json::json!({"type": "leastPing"})),
                        )
                    } else {
                        (
                            "auto".to_string(),
                            vec!["proxy".to_string()],
                            serde_json::json!({"type": "leastPing"}),
                        )
                    };
                    // The core runs the raw outbounds; the node only needs one member
                    // Lumen can read for its address and protocol. The first outbound
                    // alone used to decide, and one with settings Lumen does not model
                    // (sockopt, mux) dropped the whole auto server from the list.
                    let parsed = representative(&proxy_outbounds, remarks, subscription);
                    match parsed {
                        Ok(mut node) => {
                            if let Some(rem) = remarks {
                                node.name = rem.to_string();
                            }
                            let observatory = root.get("observatory").cloned();
                            let mut burst_observatory = root.get("burstObservatory").cloned();
                            // Without health checks Xray keeps balancing onto dead servers,
                            // and leastPing/leastLoad have no data at all.
                            if observatory.is_none() && burst_observatory.is_none() {
                                burst_observatory = Some(serde_json::json!({
                                    "subjectSelector": selector,
                                    "pingConfig": {
                                        "destination": "https://www.gstatic.com/generate_204",
                                        "interval": "1m",
                                        "sampling": 1,
                                        "timeout": "5s"
                                    }
                                }));
                            }
                            node.balancer = Some(crate::domain::node::BalancerOptions {
                                tag: b_tag,
                                selector,
                                strategy,
                                observatory,
                                burst_observatory,
                                outbounds: proxy_outbounds.into_iter().cloned().collect(),
                            });
                            let _ = node.identify();
                            parsed_entries.push(Ok(node));
                        }
                        Err(e) => parsed_entries.push(Err(e)),
                    }
                } else {
                    // Single server config
                    let mut entry = (*proxy_outbounds[0]).clone();
                    if let (Some(rem), Some(obj)) = (remarks, entry.as_object_mut()) {
                        obj.insert("remarks".to_string(), serde_json::json!(rem));
                    }
                    parsed_entries.push(super::xray_json::parse(&entry, subscription));
                }
            } else {
                // Root is itself a single outbound
                if ![Some("freedom"), Some("blackhole"), Some("dns")]
                    .contains(&root["protocol"].as_str())
                {
                    parsed_entries.push(super::xray_json::parse(&root, subscription));
                }
            }
        }
        parsed_entries
    } else if text.contains("proxies:") {
        let json: serde_json::Value =
            serde_saphyr::from_str(text).map_err(|_| BackendError::invalid())?;
        clash_rules = json.get("rules").cloned();
        let values = json["proxies"]
            .as_array()
            .ok_or_else(BackendError::invalid)?;
        if values.len() > MAX_NODES {
            return Err(BackendError::invalid());
        }
        values
            .iter()
            .map(|v| super::clash::parse(v, subscription))
            .collect()
    } else {
        let mut lines = Vec::new();
        for line in text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
        {
            match super::routing::line_routing(line) {
                Some(super::routing::LineRouting::Off) => routing_off = true,
                Some(super::routing::LineRouting::Profile(profile)) => {
                    incy_profiles.push(profile);
                }
                None => lines.push(line.to_string()),
            }
        }
        if lines.len() > MAX_NODES {
            return Err(BackendError::invalid());
        }
        lines
            .iter()
            .map(|line| uri::parse(line, subscription))
            .collect()
    };
    let mut result = ParsedSubscription {
        nodes: vec![],
        diagnostics: vec![],
        rules: vec![],
        routing_off,
    };
    // An id is the node's configuration without its name. Two entries that differ
    // only in their name (an auto server for every region over the same members,
    // say) are both listed: the later one gets an id derived from its name. Only
    // an exact repeat is dropped.
    let mut seen: HashMap<String, HashSet<String>> = HashMap::new();
    for (index, entry) in entries.into_iter().enumerate() {
        match entry {
            Ok(mut node) => {
                let names = seen.entry(node.id.clone()).or_default();
                if names.contains(&node.name) {
                    continue;
                }
                if !names.is_empty() {
                    node.id = named_id(&node.id, &node.name);
                }
                names.insert(node.name.clone());
                result.nodes.push(node);
            }
            Err(error) => result.diagnostics.push(ImportDiagnostic {
                index,
                code: error.code,
            }),
        }
    }
    if result.nodes.is_empty() {
        return Err(BackendError::invalid());
    }
    if !routing_off {
        if clash_rules
            .as_ref()
            .is_some_and(|rules| rules.as_array().is_some_and(|rules| !rules.is_empty()))
        {
            result.rules = super::routing::from_clash(
                clash_rules.as_ref().unwrap(),
                subscription,
                &result.nodes,
            );
        } else if !incy_profiles.is_empty() {
            result.rules =
                super::routing::from_profiles(&incy_profiles, subscription, &result.nodes);
        }
    }
    Ok(result)
}

/// The member an auto server is shown as: the first outbound Lumen can read, in
/// the subscription's order, so an auto server that parsed before keeps its id.
/// Fails with the first error only when no member is readable.
fn representative(
    outbounds: &[&serde_json::Value],
    remarks: Option<&str>,
    subscription: &str,
) -> Result<VpnNode> {
    let mut first_error = None;
    for outbound in outbounds {
        let mut entry = (*outbound).clone();
        if let (Some(rem), Some(obj)) = (remarks, entry.as_object_mut()) {
            obj.insert("remarks".to_string(), serde_json::json!(rem));
        }
        match super::xray_json::parse(&entry, subscription) {
            Ok(node) => return Ok(node),
            Err(error) => {
                first_error.get_or_insert(error);
            }
        }
    }
    Err(first_error.unwrap_or_else(BackendError::invalid))
}

/// A stable id for an entry whose configuration repeats an earlier one under
/// another name.
fn named_id(id: &str, name: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(id.as_bytes());
    hasher.update([0]);
    hasher.update(name.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// An outbound whose only server is loopback or unspecified cannot carry traffic.
fn is_placeholder(outbound: &serde_json::Value) -> bool {
    let settings = &outbound["settings"];
    let address = settings["vnext"][0]["address"]
        .as_str()
        .or_else(|| settings["servers"][0]["address"].as_str())
        .or_else(|| settings["address"].as_str());
    address.is_some_and(|address| {
        let host = address.trim_matches(['[', ']']);
        host.eq_ignore_ascii_case("localhost")
            || host
                .parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback() || ip.is_unspecified())
    })
}
