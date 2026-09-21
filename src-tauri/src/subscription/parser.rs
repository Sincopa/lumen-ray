use super::uri;
use crate::domain::{
    errors::{BackendError, Result},
    node::VpnNode,
};
use serde::Serialize;
use std::collections::HashSet;

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
    let entries: Vec<Result<VpnNode>> = if text.starts_with(['{', '[']) {
        let json: serde_json::Value =
            serde_json::from_str(text).map_err(|_| BackendError::invalid())?;
        let roots = json
            .as_array()
            .cloned()
            .unwrap_or_else(|| vec![json.clone()]);
        let mut parsed_entries = Vec::new();
        for root in roots {
            if parsed_entries.len() >= MAX_NODES {
                break;
            }
            if let Some(outbounds) = root.get("outbounds").and_then(|v| v.as_array()) {
                let remarks = root.get("remarks").and_then(|r| r.as_str());
                let proxy_outbounds: Vec<serde_json::Value> = outbounds
                    .iter()
                    .filter(|o| ![Some("freedom"), Some("blackhole"), Some("dns")].contains(&o["protocol"].as_str()))
                    .cloned()
                    .collect();

                if proxy_outbounds.is_empty() {
                    continue;
                }

                let has_balancers = root.get("routing")
                    .and_then(|r| r.get("balancers"))
                    .and_then(|b| b.as_array())
                    .is_some_and(|b| !b.is_empty());

                if has_balancers || proxy_outbounds.len() > 1 {
                    // Auto / Balanced node representing a cluster of servers
                    let mut first_entry = proxy_outbounds[0].clone();
                    if let (Some(rem), Some(obj)) = (remarks, first_entry.as_object_mut()) {
                        obj.insert("remarks".to_string(), serde_json::json!(rem));
                    }
                    let parsed = super::xray_json::parse(&first_entry, subscription);
                    match parsed {
                        Ok(mut node) => {
                            if let Some(rem) = remarks {
                                node.name = rem.to_string();
                            }
                            let (b_tag, selector, strategy) = if let Some(b) = root.get("routing")
                                .and_then(|r| r.get("balancers"))
                                .and_then(|b| b.as_array())
                                .and_then(|a| a.first())
                            {
                                (
                                    b["tag"].as_str().unwrap_or("auto").to_string(),
                                    b["selector"].as_array().map(|arr| arr.iter().filter_map(|s| s.as_str().map(String::from)).collect()).unwrap_or_else(|| vec!["proxy".to_string()]),
                                    b.get("strategy").cloned().unwrap_or_else(|| serde_json::json!({"type": "leastPing"})),
                                )
                            } else {
                                (
                                    "auto".to_string(),
                                    vec!["proxy".to_string()],
                                    serde_json::json!({"type": "leastPing"}),
                                )
                            };
                            node.balancer = Some(crate::domain::node::BalancerOptions {
                                tag: b_tag,
                                selector,
                                strategy,
                                observatory: root.get("observatory").cloned(),
                                outbounds: proxy_outbounds,
                            });
                            let _ = node.identify();
                            parsed_entries.push(Ok(node));
                        }
                        Err(e) => parsed_entries.push(Err(e)),
                    }
                } else {
                    // Single server config
                    let mut entry = proxy_outbounds[0].clone();
                    if let (Some(rem), Some(obj)) = (remarks, entry.as_object_mut()) {
                        obj.insert("remarks".to_string(), serde_json::json!(rem));
                    }
                    parsed_entries.push(super::xray_json::parse(&entry, subscription));
                }
            } else {
                // Root is itself a single outbound
                if ![Some("freedom"), Some("blackhole"), Some("dns")].contains(&root["protocol"].as_str()) {
                    parsed_entries.push(super::xray_json::parse(&root, subscription));
                }
            }
        }
        parsed_entries
    } else if text.contains("proxies:") {
        let json: serde_json::Value =
            serde_saphyr::from_str(text).map_err(|_| BackendError::invalid())?;
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
        let lines: Vec<_> = text
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.trim().starts_with('#'))
            .collect();
        if lines.len() > MAX_NODES {
            return Err(BackendError::invalid());
        }
        lines
            .iter()
            .map(|line| uri::parse(line.trim(), subscription))
            .collect()
    };
    let mut result = ParsedSubscription {
        nodes: vec![],
        diagnostics: vec![],
    };
    let mut seen = HashSet::new();
    for (index, entry) in entries.into_iter().enumerate() {
        match entry {
            Ok(node) => {
                if seen.insert(node.id.clone()) {
                    result.nodes.push(node);
                }
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
    Ok(result)
}
