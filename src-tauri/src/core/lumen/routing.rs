use super::{
    outbound::{Destination, Outbound},
    unsupported, validate_node,
};
use crate::{
    domain::errors::Result,
    domain::node::NodeIndex,
    storage::repository::{Repository, RuleMatcher},
};
use std::{borrow::Cow, collections::HashMap, net::IpAddr};

/// Compiled once per rule update and consulted once per flow, never per packet.
pub struct RoutingTable {
    suffixes: HashMap<String, (usize, usize)>,
    networks: HashMap<(IpAddr, u8), (usize, usize)>,
    v4_prefixes: Vec<u8>,
    v6_prefixes: Vec<u8>,
    pub outbounds: Vec<Outbound>,
    default: usize,
    bypass_lan: bool,
}
impl RoutingTable {
    pub fn compile(repo: &Repository, selected: &str) -> Result<Self> {
        if !matches!(repo.routing_mode.as_str(), "rules" | "global") {
            return Err(unsupported());
        }
        let node = repo
            .nodes
            .iter()
            .find(|n| n.id == selected)
            .ok_or_else(unsupported)?;
        validate_node(node)?;
        let mut table = Self {
            suffixes: HashMap::new(),
            networks: HashMap::new(),
            v4_prefixes: Vec::new(),
            v6_prefixes: Vec::new(),
            outbounds: vec![
                Outbound::Direct,
                Outbound::Block,
                Outbound::from_node(node).ok_or_else(unsupported)?,
            ],
            default: if repo.routing_mode == "rules" { 0 } else { 2 },
            bypass_lan: repo.settings.bypass_lan,
        };
        if repo.routing_mode != "rules" {
            return Ok(table);
        }
        let mut nodes = None;
        for (priority, rule) in crate::services::catalog_service::ordered_rules(&repo.rules)
            .into_iter()
            .filter(|r| r.enabled)
            .enumerate()
        {
            let target = match rule.server_id.as_str() {
                "direct" => 0,
                "block" => 1,
                "" | "subscription" => 2,
                id if id == selected => 2,
                id => {
                    let node = nodes
                        .get_or_insert_with(|| NodeIndex::new(&repo.nodes))
                        .target(id)
                        .ok_or_else(unsupported)?;
                    validate_node(node)?;
                    table
                        .outbounds
                        .push(Outbound::from_node(node).ok_or_else(unsupported)?);
                    table.outbounds.len() - 1
                }
            };
            if rule.matcher == RuleMatcher::Final {
                continue;
            }
            if rule.matcher != RuleMatcher::Domain {
                return Err(unsupported());
            }
            let value = rule.name.trim();
            let network = value
                .split_once('/')
                .and_then(|(ip, prefix)| {
                    Some((ip.parse::<IpAddr>().ok()?, prefix.parse::<u8>().ok()?))
                })
                .or_else(|| {
                    value
                        .parse::<IpAddr>()
                        .ok()
                        .map(|ip| (ip, if ip.is_ipv4() { 32 } else { 128 }))
                });
            if let Some((ip, prefix)) = network {
                let Some(masked) = mask(ip, prefix) else {
                    return Err(unsupported());
                };
                table
                    .networks
                    .entry((masked, prefix))
                    .or_insert((priority, target));
                if ip.is_ipv4() {
                    table.v4_prefixes.push(prefix);
                } else {
                    table.v6_prefixes.push(prefix);
                }
            } else {
                let domain = value
                    .trim_end_matches('/')
                    .strip_prefix("*.")
                    .unwrap_or(value.trim_end_matches('/'));
                // Reject lossy patterns rather than silently converting them to another host.
                if domain.contains(['/', ':', '*']) || domain.is_empty() {
                    return Err(unsupported());
                }
                let host = Destination::new(domain.into(), 1)
                    .map_err(|_| unsupported())?
                    .host;
                table.suffixes.entry(host).or_insert((priority, target));
            }
        }
        table.v4_prefixes.sort_unstable();
        table.v4_prefixes.dedup();
        table.v6_prefixes.sort_unstable();
        table.v6_prefixes.dedup();
        Ok(table)
    }
    pub fn needs_ip_lookup(&self, destination: &Destination) -> bool {
        (self.bypass_lan || !self.networks.is_empty())
            && destination.host.parse::<IpAddr>().is_err()
    }
    pub fn target(&self, destination: &Destination, resolved: &[IpAddr]) -> usize {
        let mut found = (usize::MAX, self.default);
        let host = if destination.host.is_ascii()
            && !destination
                .host
                .bytes()
                .any(|byte| byte.is_ascii_uppercase())
        {
            Cow::Borrowed(destination.host.as_str())
        } else {
            Cow::Owned(destination.host.to_lowercase())
        };
        let mut suffix: &str = host.as_ref();
        loop {
            if let Some(value) = self.suffixes.get(suffix) {
                if value.0 < found.0 {
                    found = *value;
                }
            }
            match suffix.split_once('.') {
                Some((_, next)) => suffix = next,
                None => break,
            }
        }
        for ip in destination
            .host
            .parse::<IpAddr>()
            .into_iter()
            .chain(resolved.iter().copied())
        {
            if self.bypass_lan && private(ip) {
                return 0;
            }
            let prefixes = if ip.is_ipv4() {
                &self.v4_prefixes
            } else {
                &self.v6_prefixes
            };
            for &prefix in prefixes {
                if let Some(value) =
                    mask(ip, prefix).and_then(|ip| self.networks.get(&(ip, prefix)))
                {
                    if value.0 < found.0 {
                        found = *value;
                    }
                }
            }
        }
        found.1
    }
}
fn mask(ip: IpAddr, prefix: u8) -> Option<IpAddr> {
    match ip {
        IpAddr::V4(ip) if prefix <= 32 => Some(IpAddr::V4(
            (u32::from(ip) & u32::MAX.checked_shl(32 - prefix as u32).unwrap_or(0)).into(),
        )),
        IpAddr::V6(ip) if prefix <= 128 => Some(IpAddr::V6(
            (u128::from(ip) & u128::MAX.checked_shl(128 - prefix as u32).unwrap_or(0)).into(),
        )),
        _ => None,
    }
}
fn private(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => ip.is_loopback() || ip.is_private() || ip.is_link_local(),
        IpAddr::V6(ip) => ip.is_loopback() || ip.is_unique_local() || ip.is_unicast_link_local(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full_scan(table: &RoutingTable, destination: &Destination, resolved: &[IpAddr]) -> usize {
        let mut found = (usize::MAX, table.default);
        let host = destination.host.to_lowercase();
        let mut suffix = host.as_str();
        loop {
            if let Some(value) = table.suffixes.get(suffix) {
                if value.0 < found.0 {
                    found = *value;
                }
            }
            match suffix.split_once('.') {
                Some((_, next)) => suffix = next,
                None => break,
            }
        }
        for ip in destination
            .host
            .parse::<IpAddr>()
            .into_iter()
            .chain(resolved.iter().copied())
        {
            if table.bypass_lan && private(ip) {
                return 0;
            }
            for prefix in 0..=if ip.is_ipv4() { 32 } else { 128 } {
                if let Some(value) =
                    mask(ip, prefix).and_then(|ip| table.networks.get(&(ip, prefix)))
                {
                    if value.0 < found.0 {
                        found = *value;
                    }
                }
            }
        }
        found.1
    }

    #[test]
    fn sparse_prefix_routing_matches_full_scan() {
        use crate::storage::repository::Rule;
        let node = crate::subscription::uri::parse("socks5://proxy.example:1080", "sub").unwrap();
        let mut repo = Repository {
            nodes: vec![node],
            routing_mode: "rules".into(),
            ..Default::default()
        };
        repo.rules = [
            "example.org",
            "192.0.2.0/24",
            "0.0.0.0/0",
            "2001:db8::/32",
            "::/0",
            "192.0.2.0/24",
        ]
        .into_iter()
        .enumerate()
        .map(|(i, name)| Rule {
            id: i.to_string(),
            name: name.into(),
            matcher: RuleMatcher::Domain,
            server_id: if i % 2 == 0 { "block" } else { "direct" }.into(),
            enabled: true,
            preset_id: None,
            is_domain: true,
        })
        .collect();
        for bypass in [false, true] {
            repo.settings.bypass_lan = bypass;
            let table = RoutingTable::compile(&repo, &repo.nodes[0].id).unwrap();
            assert_eq!(table.v4_prefixes, [0, 24]);
            assert_eq!(table.v6_prefixes, [0, 32]);
            for host in [
                "EXAMPLE.ORG",
                "child.example.org",
                "other.org",
                "192.0.2.3",
                "198.51.100.2",
                "127.0.0.1",
                "2001:db8::123",
                "2001:db9::1",
                "::1",
            ] {
                let destination = Destination {
                    host: host.into(),
                    port: 443,
                };
                for addresses in [
                    vec![],
                    vec!["192.0.2.1".parse().unwrap()],
                    vec!["2001:db8::1".parse().unwrap(), "127.0.0.1".parse().unwrap()],
                ] {
                    assert_eq!(
                        table.target(&destination, &addresses),
                        full_scan(&table, &destination, &addresses)
                    );
                }
            }
        }
    }
}
