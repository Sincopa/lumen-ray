//! Shared persisted DNS policy and validation; backends compile it separately.
use crate::domain::errors::{BackendError, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DnsPolicy {
    pub enabled: bool,
    pub direct: String,
    pub proxy: String,
    pub fallback: Vec<String>,
    pub bootstrap: String,
    pub ipv6: String,
}
impl Default for DnsPolicy {
    fn default() -> Self {
        Self {
            enabled: false,
            direct: "system".into(),
            proxy: "https://1.1.1.1/dns-query".into(),
            fallback: vec![],
            bootstrap: "1.1.1.1".into(),
            ipv6: "auto".into(),
        }
    }
}
fn invalid() -> BackendError {
    BackendError::new("INVALID_CONFIG", "Invalid DNS policy")
}
pub fn valid_ip(input: &str) -> bool {
    input.parse::<std::net::IpAddr>().is_ok_and(|ip| {
        !ip.is_unspecified() && !ip.is_multicast() && ip != std::net::Ipv4Addr::BROADCAST
    })
}
pub fn valid_resolver(input: &str) -> bool {
    if valid_ip(input) {
        return true;
    }
    url::Url::parse(input).is_ok_and(|url| {
        url.scheme() == "https"
            && url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none()
            && url.fragment().is_none()
            && input.len() <= 1024
    })
}
impl DnsPolicy {
    pub fn validate(&self) -> Result<()> {
        if (self.direct != "system" && !valid_resolver(&self.direct))
            || !valid_resolver(&self.proxy)
            || !valid_ip(&self.bootstrap)
            || self.fallback.len() > 3
            || self.fallback.iter().any(|s| !valid_resolver(s))
            || !["auto", "ipv4"].contains(&self.ipv6.as_str())
        {
            return Err(invalid());
        }
        Ok(())
    }
}
