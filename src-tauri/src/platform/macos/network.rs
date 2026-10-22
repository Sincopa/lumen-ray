//! Read-only network state from the system tools; Lumen installs no routes on macOS.
use crate::domain::errors::{BackendError, Result};
use std::net::IpAddr;

/// Interfaces of other VPNs, Apple services and loopback.
pub(crate) fn is_physical(interface: &str) -> bool {
    ![
        "lo", "utun", "ipsec", "ppp", "gif", "stf", "awdl", "llw", "anpi", "ap",
    ]
    .iter()
    .any(|prefix| {
        interface
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.chars().all(|c| c.is_ascii_digit()))
    })
}
fn output(program: &str, args: &[&str]) -> Option<String> {
    let output = std::process::Command::new(program)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}
#[derive(Debug, Default)]
struct Resolver {
    scoped: bool,
    domain: bool,
    interface: Option<String>,
    servers: Vec<IpAddr>,
}
fn resolvers(text: &str) -> Vec<Resolver> {
    let mut list: Vec<Resolver> = Vec::new();
    let mut scoped = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with("DNS configuration") {
            scoped = line.contains("scoped");
        } else if line.starts_with("resolver #") {
            list.push(Resolver {
                scoped,
                ..Default::default()
            });
        } else if let (Some(resolver), Some((key, value))) = (list.last_mut(), line.split_once(':'))
        {
            let value = value.trim();
            match key.trim() {
                "domain" => resolver.domain = true,
                "if_index" => {
                    resolver.interface = value
                        .split_once('(')
                        .and_then(|(_, name)| name.strip_suffix(')'))
                        .map(str::to_owned)
                }
                // Scoped link-local servers (`fe80::1%en0`) do not parse and are skipped.
                key if key.starts_with("nameserver[") => {
                    if let Ok(ip) = value.parse::<IpAddr>() {
                        if !ip.is_loopback() && !ip.is_unspecified() {
                            resolver.servers.push(ip);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    list
}
/// Scoped resolvers first: while another VPN is primary, the default one is its tunnel.
fn select_dns(text: &str) -> Option<Vec<String>> {
    let list = resolvers(text);
    let usable = |r: &&Resolver| {
        !r.domain && !r.servers.is_empty() && r.interface.as_deref().is_none_or(is_physical)
    };
    list.iter()
        .filter(|r| r.scoped && r.interface.is_some())
        .find(usable)
        .or_else(|| list.iter().filter(|r| !r.scoped).find(usable))
        .map(|r| {
            let mut servers: Vec<String> = Vec::new();
            for ip in &r.servers {
                if !servers.contains(&ip.to_string()) {
                    servers.push(ip.to_string());
                }
            }
            servers
        })
}
/// DNS configured on the active physical network.
pub fn system_dns() -> Result<Vec<String>> {
    output("/usr/sbin/scutil", &["--dns"])
        .and_then(|text| select_dns(&text))
        .ok_or_else(|| {
            BackendError::new("DNS_UNAVAILABLE", "Could not read the system DNS servers")
        })
}
fn physical_ipv6_default(text: &str) -> bool {
    text.lines().any(|line| {
        let fields: Vec<&str> = line.split_whitespace().collect();
        fields.first() == Some(&"default") && fields.get(3).is_some_and(|name| is_physical(name))
    })
}
/// macOS adds a scoped `default` route for every utun, so only physical interfaces count.
/// Unused until macOS gets TUN.
#[allow(dead_code)]
pub fn physical_ipv6_available() -> bool {
    output("/usr/sbin/netstat", &["-rn", "-f", "inet6"])
        .is_some_and(|text| physical_ipv6_default(&text))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DNS: &str = "DNS configuration

resolver #1
  nameserver[0] : 198.18.0.2
  if_index : 19 (utun4)
  flags    : Request A records
  reach    : 0x00000002 (Reachable)

resolver #2
  domain   : local
  options  : mdns
  timeout  : 5
  flags    : Request A records
  reach    : 0x00000000 (Not Reachable)
  order    : 300000

DNS configuration (for scoped queries)

resolver #1
  nameserver[0] : 198.18.0.2
  if_index : 19 (utun4)
  flags    : Scoped, Request A records

resolver #2
  nameserver[0] : 192.168.2.253
  nameserver[1] : fe80::1%en0
  nameserver[2] : 2001:db8::53
  nameserver[3] : 127.0.0.1
  nameserver[4] : 192.168.2.253
  if_index : 12 (en0)
  flags    : Scoped, Request A records
  reach    : 0x00020002 (Reachable,Directly Reachable Address)
";

    #[test]
    fn dns_prefers_the_physical_scoped_resolver() {
        assert_eq!(
            select_dns(DNS).unwrap(),
            vec!["192.168.2.253".to_string(), "2001:db8::53".to_string()]
        );
    }

    #[test]
    fn dns_falls_back_to_the_default_resolver_and_skips_loopback() {
        let text = "DNS configuration\n\nresolver #1\n  nameserver[0] : 127.0.0.1\n\nresolver #2\n  nameserver[0] : 10.0.0.1\n  if_index : 4 (en1)\n";
        assert_eq!(select_dns(text).unwrap(), vec!["10.0.0.1".to_string()]);
        assert_eq!(
            select_dns(
                "DNS configuration\n\nresolver #1\n  domain : local\n  nameserver[0] : 10.0.0.1\n"
            ),
            None
        );
        assert_eq!(select_dns("No DNS configuration available\n"), None);
    }

    #[test]
    fn ipv6_defaults_on_tunnels_are_not_physical() {
        let tunnels = "Internet6:\nDestination  Gateway  Flags  Netif Expire\ndefault  fe80::%utun0  UGcIg  utun0\ndefault  fe80::%utun1  UGcIg  utun1\n::1  ::1  UHL  lo0\n";
        assert!(!physical_ipv6_default(tunnels));
        let wifi = format!("{tunnels}default  fe80::1%en0  UGc  en0\n");
        assert!(physical_ipv6_default(&wifi));
    }

    #[test]
    fn interface_classes() {
        for name in ["en0", "en10", "bridge0"] {
            assert!(is_physical(name), "{name}");
        }
        for name in [
            "lo0", "utun3", "ipsec0", "awdl0", "llw0", "anpi1", "ap1", "ppp0",
        ] {
            assert!(!is_physical(name), "{name}");
        }
    }
}
