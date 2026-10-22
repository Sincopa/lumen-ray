//! Interface address polling; getifaddrs is a cheap syscall, no helper process.
use std::sync::atomic::{AtomicU64, Ordering};
static REVISION: AtomicU64 = AtomicU64::new(0);

/// Routable addresses of up physical interfaces; link-local and utun are ignored.
fn physical_addresses() -> Option<Vec<(String, Vec<u8>)>> {
    let mut list = std::ptr::null_mut();
    if unsafe { libc::getifaddrs(&mut list) } != 0 {
        return None;
    }
    let mut addresses = Vec::new();
    let mut entry = list;
    unsafe {
        while let Some(item) = entry.as_ref() {
            let up = (libc::IFF_UP | libc::IFF_RUNNING) as u32;
            if !item.ifa_addr.is_null() && item.ifa_flags & up == up {
                let name = std::ffi::CStr::from_ptr(item.ifa_name).to_string_lossy();
                let ip = match (*item.ifa_addr).sa_family as i32 {
                    libc::AF_INET => {
                        let raw = (*item.ifa_addr.cast::<libc::sockaddr_in>()).sin_addr.s_addr;
                        Some(std::net::IpAddr::from(u32::from_be(raw).to_be_bytes()))
                    }
                    libc::AF_INET6 => Some(std::net::IpAddr::from(
                        (*item.ifa_addr.cast::<libc::sockaddr_in6>())
                            .sin6_addr
                            .s6_addr,
                    )),
                    _ => None,
                };
                if let Some(ip) = ip.filter(routable) {
                    if super::network::is_physical(&name) {
                        let bytes = match ip {
                            std::net::IpAddr::V4(v4) => v4.octets().to_vec(),
                            std::net::IpAddr::V6(v6) => v6.octets().to_vec(),
                        };
                        addresses.push((name.into_owned(), bytes));
                    }
                }
            }
            entry = item.ifa_next;
        }
        libc::freeifaddrs(list);
    }
    addresses.sort();
    Some(addresses)
}
fn routable(ip: &std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => {
            !v4.is_loopback() && !v4.is_link_local() && !v4.is_unspecified()
        }
        std::net::IpAddr::V6(v6) => {
            !v6.is_loopback() && !v6.is_unspecified() && (v6.segments()[0] & 0xffc0) != 0xfe80
        }
    }
}
fn hash(addresses: &[(String, Vec<u8>)]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    addresses.hash(&mut hasher);
    hasher.finish()
}

/// Physical interfaces only: tunnel changes must not trigger a restart loop.
pub fn physical_fingerprint() -> Option<u64> {
    use std::hash::{Hash, Hasher};
    let addresses = physical_addresses().filter(|list| !list.is_empty())?;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    hash(&addresses).hash(&mut hasher);
    // Include DHCP DNS changes even if the interface address stays the same.
    crate::platform::network::system_dns()
        .ok()
        .hash(&mut hasher);
    Some(hasher.finish())
}
pub struct NetworkWatch;
impl NetworkWatch {
    pub fn new() -> Self {
        static STARTED: std::sync::Once = std::sync::Once::new();
        STARTED.call_once(|| {
            let spawned = std::thread::Builder::new()
                .name("lumen-network-watch".into())
                .spawn(|| {
                    let mut last = physical_addresses().map(|list| hash(&list));
                    loop {
                        std::thread::sleep(std::time::Duration::from_secs(2));
                        let now = physical_addresses().map(|list| hash(&list));
                        if now != last {
                            last = now;
                            REVISION.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                });
            if spawned.is_err() {
                log::warn!(
                    "Network event registration failed; periodic health checks remain enabled"
                );
            }
        });
        Self
    }
    pub fn revision(&self) -> u64 {
        REVISION.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_local_and_loopback_are_not_networks() {
        for ip in ["127.0.0.1", "169.254.10.1", "::1", "fe80::1", "0.0.0.0"] {
            assert!(!routable(&ip.parse().unwrap()), "{ip}");
        }
        for ip in ["192.168.1.5", "10.0.0.2", "2001:db8::1", "fd00::1"] {
            assert!(routable(&ip.parse().unwrap()), "{ip}");
        }
    }

    #[test]
    fn addresses_are_readable() {
        assert!(physical_addresses().is_some());
        let watch = NetworkWatch::new();
        let _ = watch.revision();
    }
}
