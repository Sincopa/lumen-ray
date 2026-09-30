use crate::{
    domain::errors::Result,
    storage::repository::{Repository, RuleMatcher},
};
use serde_json::{json, Value};

#[cfg(test)]
use crate::core::dns::valid_resolver;
#[cfg(test)]
use crate::core::dns::DnsPolicy;

fn address(value: &str) -> String {
    if value.starts_with("https://") {
        value.into()
    } else if value.contains(':') {
        format!("tcp://[{value}]")
    } else {
        format!("tcp://{value}")
    }
}
pub fn build(
    repo: &Repository,
    target_field: &str,
    target: &str,
    rules: &mut Vec<Value>,
) -> Result<Value> {
    let p = &repo.settings.dns_policy;
    p.validate()?;
    let direct = if p.direct == "system" {
        #[cfg(windows)]
        {
            crate::platform::network::system_dns()?
        }
        #[cfg(not(windows))]
        {
            vec![p.bootstrap.clone()]
        }
    } else {
        vec![p.direct.clone()]
    };
    let mut proxy = vec![p.proxy.clone()];
    proxy.extend(p.fallback.clone());
    let mut servers = Vec::new();
    let mut bootstrap_domains: Vec<String> = repo
        .nodes
        .iter()
        .filter(|n| n.address.parse::<std::net::IpAddr>().is_err())
        .map(|n| format!("full:{}", n.address))
        .collect();
    for resolver in direct.iter().chain(proxy.iter()) {
        if let Ok(url) = url::Url::parse(resolver) {
            if let Some(host) = url
                .host_str()
                .filter(|h| h.parse::<std::net::IpAddr>().is_err())
            {
                bootstrap_domains.push(format!("full:{host}"));
            }
        }
    }
    bootstrap_domains.sort();
    bootstrap_domains.dedup();
    if !bootstrap_domains.is_empty() {
        servers.push(json!({"address":address(&p.bootstrap),"domains":bootstrap_domains,"skipFallback":true,"finalQuery":true,"tag":"dns-bootstrap"}));
    }
    let mut add_group = |addresses: &[String], domains: Vec<String>, tag: &str, scoped: bool| {
        for (i, resolver) in addresses.iter().enumerate() {
            let mut server = json!({"address":address(resolver),"tag":tag,"skipFallback":scoped,"finalQuery":scoped && i+1 == addresses.len()});
            if scoped {
                server["domains"] = json!(domains);
            }
            servers.push(server);
        }
    };
    add_group(
        &direct,
        vec![
            "domain:local".into(),
            "domain:lan".into(),
            "full:localhost".into(),
        ],
        "dns-direct",
        true,
    );
    if repo.routing_mode == "rules" {
        for rule in crate::services::catalog_service::ordered_rules(&repo.rules)
            .into_iter()
            .filter(|r| r.enabled)
        {
            let domain = match rule.matcher {
                RuleMatcher::Geosite => Some(format!(
                    "geosite:{}",
                    crate::storage::repository::normalize_geo_tag(&rule.name, "geosite")?
                )),
                RuleMatcher::Domain
                    if !rule.name.contains('/')
                        && rule.name.parse::<std::net::IpAddr>().is_err() =>
                {
                    Some(super::config::domain_rule(&rule.name)?)
                }
                _ => None,
            };
            if let Some(domain) = domain {
                let local = rule.server_id == "direct";
                add_group(
                    if local { &direct } else { &proxy },
                    vec![domain],
                    if local { "dns-direct" } else { "dns-proxy" },
                    true,
                );
            }
        }
    }
    if repo.routing_mode == "rules" {
        add_group(&direct, vec![], "dns-direct", false);
    } else {
        add_group(&proxy, vec![], "dns-proxy", false);
    }
    rules.insert(
        0,
        json!({"type":"field","inboundTag":["dns-direct","dns-bootstrap"],"outboundTag":"direct"}),
    );
    rules.insert(
        1,
        json!({"type":"field","inboundTag":["dns-proxy"],target_field:target}),
    );
    if p.ipv6 == "ipv4" {
        rules.push(json!({"type":"field","ip":["::/0"],"outboundTag":"block"}));
    }
    Ok(
        json!({"servers":servers,"queryStrategy":if p.ipv6=="ipv4" {"UseIPv4"} else {"UseIP"},"disableFallbackIfMatch":true,"useSystemHosts":true}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_policy_without_accepting_credentials_or_insecure_doh() {
        assert!(DnsPolicy::default().validate().is_ok());
        for value in [
            "http://1.1.1.1/dns-query",
            "https://user:pass@example.org/dns",
            "0.0.0.0",
            "224.0.0.1",
        ] {
            assert!(!valid_resolver(value));
        }
        assert!(valid_resolver("https://dns.example.org/dns-query"));
    }
    #[test]
    fn fallback_stays_on_same_path_and_scoped_queries_terminate() {
        let mut repo = Repository::default();
        repo.settings.dns_policy.direct = "9.9.9.9".into();
        repo.settings.dns_policy.fallback = vec!["8.8.8.8".into()];
        let mut rules = vec![];
        let config = build(&repo, "outboundTag", "proxy", &mut rules).unwrap();
        let servers = config["servers"].as_array().unwrap();
        assert!(servers
            .iter()
            .filter(|s| s["skipFallback"] == false)
            .all(|s| s["tag"] == "dns-proxy"));
        assert_eq!(servers[0]["finalQuery"], true);
    }
}
