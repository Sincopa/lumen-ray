use windows_sys::Win32::{
    NetworkManagement::{
        IpHelper::{
            ConvertInterfaceAliasToLuid, FreeMibTable, GetAdaptersAddresses, GetIpForwardTable2,
            GAA_FLAG_INCLUDE_GATEWAYS, IP_ADAPTER_ADDRESSES_LH,
        },
        Ndis::{IfOperStatusUp, NET_LUID_LH},
    },
    Networking::WinSock::{AF_INET, AF_INET6, AF_UNSPEC, SOCKADDR_IN, SOCKADDR_IN6},
};

const LUMEN_TUN_ALIAS: [u16; 9] = [
    b'L' as u16,
    b'u' as u16,
    b'm' as u16,
    b'e' as u16,
    b'n' as u16,
    b'T' as u16,
    b'u' as u16,
    b'n' as u16,
    0,
];

fn tunnel_luid() -> Option<NET_LUID_LH> {
    let mut luid: NET_LUID_LH = unsafe { std::mem::zeroed() };
    if unsafe { ConvertInterfaceAliasToLuid(LUMEN_TUN_ALIAS.as_ptr(), &mut luid) } == 0 {
        Some(luid)
    } else {
        None
    }
}

/// DNS configured on the active physical adapter, before creating LumenTun.
/// Do not silently replace the user's system resolver with a public provider.
pub fn system_dns() -> crate::domain::errors::Result<Vec<String>> {
    use crate::domain::errors::BackendError;
    use windows_sys::Win32::Foundation::ERROR_BUFFER_OVERFLOW;
    let error = || BackendError::new("DNS_UNAVAILABLE", "Could not read the system DNS servers");
    let mut bytes = 16_384u32;
    for _ in 0..3 {
        // The Windows API returns aligned structures, so allocate in u64 units.
        let mut buffer = vec![0u64; (bytes as usize).div_ceil(8)];
        let mut size = (buffer.len() * 8) as u32;
        let status = unsafe {
            GetAdaptersAddresses(
                AF_UNSPEC as u32,
                GAA_FLAG_INCLUDE_GATEWAYS,
                std::ptr::null(),
                buffer.as_mut_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>(),
                &mut size,
            )
        };
        if status == ERROR_BUFFER_OVERFLOW {
            bytes = size;
            continue;
        }
        if status != 0 {
            return Err(error());
        }
        let mut adapter = buffer.as_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>();
        let mut best: Option<(u32, Vec<String>)> = None;
        unsafe {
            while !adapter.is_null() {
                let entry = &*adapter;
                let metric = entry.Ipv4Metric;
                if entry.OperStatus == IfOperStatusUp
                    && !entry.FirstGatewayAddress.is_null()
                    && !entry.FirstDnsServerAddress.is_null()
                    && best.as_ref().is_none_or(|(previous, _)| metric < *previous)
                {
                    let name = if entry.FriendlyName.is_null() {
                        String::new()
                    } else {
                        let len = (0..256)
                            .find(|&i| *entry.FriendlyName.add(i) == 0)
                            .unwrap_or(256);
                        String::from_utf16_lossy(std::slice::from_raw_parts(
                            entry.FriendlyName,
                            len,
                        ))
                    };
                    if !name.to_ascii_lowercase().contains("lumentun") {
                        let mut servers = Vec::new();
                        let mut dns = entry.FirstDnsServerAddress;
                        while !dns.is_null() {
                            let address = (*dns).Address.lpSockaddr;
                            if !address.is_null() {
                                let ip = match (*address).sa_family {
                                    AF_INET => {
                                        Some(std::net::IpAddr::V4(std::net::Ipv4Addr::from(
                                            (*(address.cast::<SOCKADDR_IN>()))
                                                .sin_addr
                                                .S_un
                                                .S_addr
                                                .to_ne_bytes(),
                                        )))
                                    }
                                    AF_INET6 => {
                                        Some(std::net::IpAddr::V6(std::net::Ipv6Addr::from(
                                            (*(address.cast::<SOCKADDR_IN6>())).sin6_addr.u.Byte,
                                        )))
                                    }
                                    _ => None,
                                };
                                if let Some(ip) = ip.filter(|ip| !ip.is_unspecified()) {
                                    servers.push(ip.to_string());
                                }
                            }
                            dns = (*dns).Next;
                        }
                        if !servers.is_empty() {
                            best = Some((metric, servers));
                        }
                    }
                }
                adapter = entry.Next;
            }
        }
        return best.map(|(_, servers)| servers).ok_or_else(error);
    }
    Err(error())
}
/// Read-only confirmation that Xray installed the default route on its own adapter.
/// An unreadable table is not treated as ready.
pub fn tunnel_ready() -> bool {
    tunnel_route_exists(AF_INET) == Some(true)
}

/// Whether a physical adapter has an IPv6 default route. Without one, a tunnel
/// `::/0` invents IPv6 connectivity: browsers then prefer AAAA records, direct
/// rules have no IPv6 path, and Xray cannot match processes on TUN IPv6 flows.
pub fn physical_ipv6_available() -> bool {
    let tunnel = tunnel_luid().map(|luid| unsafe { luid.Value });
    unsafe {
        let mut table = std::ptr::null_mut();
        if GetIpForwardTable2(AF_INET6, &mut table) != 0 || table.is_null() {
            return false;
        }
        let entries =
            std::slice::from_raw_parts((*table).Table.as_ptr(), (*table).NumEntries as usize);
        let found = entries.iter().any(|r| {
            r.DestinationPrefix.PrefixLength == 0 && Some(r.InterfaceLuid.Value) != tunnel
        });
        FreeMibTable(table as _);
        found
    }
}
/// Default route owned by the tunnel adapter. Other prefixes and other
/// interfaces stay untouched.
pub(crate) fn is_lumen_default_route(
    interface_luid: u64,
    prefix_length: u8,
    tunnel_luid: u64,
) -> bool {
    interface_luid == tunnel_luid && prefix_length == 0
}

/// `Some(true)` means a Lumen default route is present. `Some(false)` means the
/// table was readable and that route is gone, including when the adapter itself
/// is gone. `None` means the table could not be read: that is not evidence the
/// route is gone, and callers must not tear the tunnel down on it.
fn tunnel_route_exists(family: u16) -> Option<bool> {
    let Some(luid) = tunnel_luid() else {
        return Some(false);
    };
    unsafe {
        let mut table = std::ptr::null_mut();
        if GetIpForwardTable2(family, &mut table) != 0 || table.is_null() {
            return None;
        }
        let entries =
            std::slice::from_raw_parts((*table).Table.as_ptr(), (*table).NumEntries as usize);
        let found = entries.iter().any(|route| {
            is_lumen_default_route(
                route.InterfaceLuid.Value,
                route.DestinationPrefix.PrefixLength,
                luid.Value,
            )
        });
        FreeMibTable(table as _);
        Some(found)
    }
}

/// Both families were read and neither still has a Lumen default route.
/// A failed read stays `false` so a cleanup path will not kill Xray on a guess.
pub(crate) fn defaults_absent_from_reads(ipv4: Option<bool>, ipv6: Option<bool>) -> bool {
    ipv4 == Some(false) && ipv6 == Some(false)
}

/// No default route remains on LumenTun. A missing adapter counts as clear:
/// Windows drops routes that belonged to an interface that no longer exists.
pub fn tunnel_defaults_absent() -> bool {
    defaults_absent_from_reads(tunnel_route_exists(AF_INET), tunnel_route_exists(AF_INET6))
}

/// Default routes are gone and, when the adapter still exists, it no longer
/// outranks a physical interface.
pub fn routes_released() -> bool {
    if !tunnel_defaults_absent() {
        return false;
    }
    let Some(luid) = tunnel_luid() else {
        return true;
    };
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        GetIpInterfaceEntry, InitializeIpInterfaceEntry, MIB_IPINTERFACE_ROW,
    };
    unsafe {
        [AF_INET, AF_INET6].into_iter().all(|family| {
            let mut interface = MIB_IPINTERFACE_ROW::default();
            InitializeIpInterfaceEntry(&mut interface);
            interface.Family = family;
            interface.InterfaceLuid = luid;
            let status = GetIpInterfaceEntry(&mut interface);
            status != 0 || (!interface.UseAutomaticMetric && interface.Metric >= 5_000)
        })
    }
}

/// Delete every default route Windows actually installed on LumenTun, then
/// push the adapter behind physical interfaces. Success is a second read of
/// the routing table, not the status of a single delete.
pub fn release_lumen_routes() -> crate::domain::errors::Result<()> {
    use crate::domain::errors::BackendError;
    let Some(luid) = tunnel_luid() else {
        return Ok(());
    };
    let tunnel = unsafe { luid.Value };
    for family in [AF_INET, AF_INET6] {
        delete_lumen_default_routes(family, tunnel)?;
    }
    lower_tunnel_priority(luid)?;
    if routes_released() {
        Ok(())
    } else {
        Err(BackendError::new(
            "TUN_FAILED",
            "Could not remove tunnel routes",
        ))
    }
}

fn delete_lumen_default_routes(family: u16, tunnel: u64) -> crate::domain::errors::Result<()> {
    use crate::domain::errors::BackendError;
    use windows_sys::Win32::Foundation::ERROR_NOT_FOUND;
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        DeleteIpForwardEntry2, MIB_IPFORWARD_ROW2,
    };
    unsafe {
        let mut table = std::ptr::null_mut();
        if GetIpForwardTable2(family, &mut table) != 0 || table.is_null() {
            return Err(BackendError::new(
                "TUN_FAILED",
                "Could not read tunnel routes",
            ));
        }
        let entries =
            std::slice::from_raw_parts((*table).Table.as_ptr(), (*table).NumEntries as usize);
        let owned: Vec<MIB_IPFORWARD_ROW2> = entries
            .iter()
            .filter(|route| {
                is_lumen_default_route(
                    route.InterfaceLuid.Value,
                    route.DestinationPrefix.PrefixLength,
                    tunnel,
                )
            })
            .copied()
            .collect();
        FreeMibTable(table as _);
        for route in &owned {
            let status = DeleteIpForwardEntry2(route);
            if status != 0 && status != ERROR_NOT_FOUND {
                return Err(BackendError::new(
                    "TUN_FAILED",
                    "Could not remove tunnel routes",
                ));
            }
        }
    }
    Ok(())
}

fn lower_tunnel_priority(luid: NET_LUID_LH) -> crate::domain::errors::Result<()> {
    use crate::domain::errors::BackendError;
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        GetIpInterfaceEntry, InitializeIpInterfaceEntry, MIB_IPINTERFACE_ROW,
    };
    for family in [AF_INET, AF_INET6] {
        let mut interface = MIB_IPINTERFACE_ROW::default();
        unsafe {
            InitializeIpInterfaceEntry(&mut interface);
        }
        interface.Family = family;
        interface.InterfaceLuid = luid;
        if unsafe { GetIpInterfaceEntry(&mut interface) } != 0 {
            continue;
        }
        if !interface.UseAutomaticMetric && interface.Metric >= 5_000 {
            continue;
        }
        interface.UseAutomaticMetric = false;
        interface.Metric = 5_000;
        if set_tunnel_interface(&mut interface) != 0 {
            return Err(BackendError::new(
                "TUN_FAILED",
                "Could not lower standby tunnel priority",
            ));
        }
    }
    Ok(())
}

/// The prepared adapter must not own default routes or outrank ordinary adapters.
pub fn standby_isolated() -> bool {
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        GetIpInterfaceEntry, InitializeIpInterfaceEntry, MIB_IPINTERFACE_ROW,
    };
    if tunnel_route_exists(AF_INET) != Some(false) || tunnel_route_exists(AF_INET6) != Some(false) {
        return false;
    }
    let Some(luid) = tunnel_luid() else {
        return false;
    };
    unsafe {
        [AF_INET, AF_INET6].into_iter().all(|family| {
            let mut interface = MIB_IPINTERFACE_ROW::default();
            InitializeIpInterfaceEntry(&mut interface);
            interface.Family = family;
            interface.InterfaceLuid = luid;
            GetIpInterfaceEntry(&mut interface) == 0
                && !interface.UseAutomaticMetric
                && interface.Metric >= 5_000
        })
    }
}

/// A prepared tunnel has an initialized interface but no system default route.
pub fn tunnel_adapter_ready() -> bool {
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        GetIpInterfaceEntry, InitializeIpInterfaceEntry, MIB_IPINTERFACE_ROW,
    };
    let Some(luid) = tunnel_luid() else {
        return false;
    };
    unsafe {
        [AF_INET, AF_INET6].into_iter().all(|family| {
            let mut row = MIB_IPINTERFACE_ROW::default();
            InitializeIpInterfaceEntry(&mut row);
            row.Family = family;
            row.InterfaceLuid = luid;
            GetIpInterfaceEntry(&mut row) == 0 && row.Connected
        })
    }
}

fn set_tunnel_interface(
    interface: &mut windows_sys::Win32::NetworkManagement::IpHelper::MIB_IPINTERFACE_ROW,
) -> u32 {
    // GetIpInterfaceEntry can return an IPv4 SitePrefixLength that the setter
    // rejects. Windows requires zero when writing an IPv4 interface row.
    if interface.Family == AF_INET {
        interface.SitePrefixLength = 0;
    }
    unsafe { windows_sys::Win32::NetworkManagement::IpHelper::SetIpInterfaceEntry(interface) }
}

/// Owns only the two default routes created for a prepared TUN session.
/// The helper drops them before it acknowledges a disconnect.
pub struct TunnelRoutes {
    luid: NET_LUID_LH,
    rows: Vec<windows_sys::Win32::NetworkManagement::IpHelper::MIB_IPFORWARD_ROW2>,
    metrics: Vec<(u16, bool, u32)>,
    active: bool,
}

impl TunnelRoutes {
    pub fn new() -> crate::domain::errors::Result<Self> {
        use crate::domain::errors::BackendError;
        use windows_sys::Win32::NetworkManagement::IpHelper::{
            GetIpInterfaceEntry, InitializeIpInterfaceEntry, MIB_IPINTERFACE_ROW,
        };
        let luid = tunnel_luid()
            .ok_or_else(|| BackendError::new("TUN_FAILED", "Tunnel adapter is unavailable"))?;
        // Windows also uses interface priority when choosing DNS and application
        // sockets. A prepared adapter must rank behind the physical network.
        for family in [AF_INET, AF_INET6] {
            let mut interface = MIB_IPINTERFACE_ROW::default();
            unsafe {
                InitializeIpInterfaceEntry(&mut interface);
            }
            interface.Family = family;
            interface.InterfaceLuid = luid;
            if unsafe { GetIpInterfaceEntry(&mut interface) } != 0 {
                return Err(BackendError::new(
                    "TUN_FAILED",
                    "Tunnel interface is not ready",
                ));
            }
            interface.UseAutomaticMetric = false;
            interface.Metric = 5_000;
            if set_tunnel_interface(&mut interface) != 0 {
                return Err(BackendError::new(
                    "TUN_FAILED",
                    "Could not lower standby tunnel priority",
                ));
            }
        }
        Ok(Self {
            luid,
            rows: Vec::new(),
            metrics: Vec::new(),
            active: false,
        })
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn activate(&mut self) -> crate::domain::errors::Result<()> {
        use crate::domain::errors::BackendError;
        use windows_sys::Win32::NetworkManagement::IpHelper::{
            CreateIpForwardEntry2, GetIpInterfaceEntry, InitializeIpForwardEntry,
            InitializeIpInterfaceEntry, MIB_IPFORWARD_ROW2, MIB_IPINTERFACE_ROW,
        };
        if self.active {
            return Ok(());
        }
        if !self.rows.is_empty() || !self.metrics.is_empty() {
            return Err(BackendError::new(
                "TUN_FAILED",
                "Tunnel route cleanup is incomplete",
            ));
        }
        let families: &[u16] = if physical_ipv6_available() {
            &[AF_INET, AF_INET6]
        } else {
            &[AF_INET]
        };
        for &family in families {
            let mut interface = MIB_IPINTERFACE_ROW::default();
            unsafe {
                InitializeIpInterfaceEntry(&mut interface);
            }
            interface.Family = family;
            interface.InterfaceLuid = self.luid;
            if unsafe { GetIpInterfaceEntry(&mut interface) } != 0 {
                let _ = self.deactivate();
                return Err(BackendError::new(
                    "TUN_FAILED",
                    "Tunnel interface is not ready",
                ));
            }
            self.metrics
                .push((family, interface.UseAutomaticMetric, interface.Metric));
            interface.UseAutomaticMetric = false;
            interface.Metric = 0;
            if set_tunnel_interface(&mut interface) != 0 {
                let _ = self.deactivate();
                return Err(BackendError::new(
                    "TUN_FAILED",
                    "Could not configure tunnel priority",
                ));
            }
            let mut route = MIB_IPFORWARD_ROW2::default();
            unsafe {
                InitializeIpForwardEntry(&mut route);
            }
            route.InterfaceLuid = self.luid;
            route.DestinationPrefix.PrefixLength = 0;
            if family == AF_INET {
                route.DestinationPrefix.Prefix.Ipv4.sin_family = family;
                route.NextHop.Ipv4.sin_family = family;
            } else {
                route.DestinationPrefix.Prefix.Ipv6.sin6_family = family;
                route.NextHop.Ipv6.sin6_family = family;
            }
            route.Metric = 0;
            if unsafe { CreateIpForwardEntry2(&route) } != 0 {
                let _ = self.deactivate();
                return Err(BackendError::new(
                    "TUN_FAILED",
                    "Could not enable tunnel route",
                ));
            }
            self.rows.push(route);
        }
        // Windows can publish the new default routes before GetBestRoute2
        // starts selecting them. A single immediate check used to reject a
        // healthy server switch and force a second, slow helper startup.
        let mut last_conflict = None;
        for attempt in 0..30 {
            last_conflict = tunnel_route_conflict();
            if last_conflict.is_none() {
                if attempt > 0 {
                    log::info!(
                        "Lumen tunnel route selection settled after {} ms",
                        attempt * 50
                    );
                }
                self.active = true;
                return Ok(());
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        if let Some(reason) = last_conflict {
            let error = route_conflict_error(reason);
            let _ = self.deactivate();
            return Err(error);
        }
        self.active = true;
        Ok(())
    }

    pub fn deactivate(&mut self) -> crate::domain::errors::Result<()> {
        // Drop every default route on the adapter Windows currently has, including
        // rows that no longer match the values passed to CreateIpForwardEntry2.
        self.active = false;
        match release_lumen_routes() {
            Ok(()) => {
                self.rows.clear();
                self.metrics.clear();
                Ok(())
            }
            Err(error) => {
                // Keep a sentinel so activate() will not install another default
                // route on top of one that is still in the table.
                if self.rows.is_empty() {
                    self.rows.push(windows_sys::Win32::NetworkManagement::IpHelper::MIB_IPFORWARD_ROW2::default());
                }
                Err(error)
            }
        }
    }
}

/// Check the route Windows would actually select, without binding it to our adapter.
/// A competing VPN can leave our defaults installed but shadow them with split routes.
pub fn verify_tunnel_routes() -> crate::domain::errors::Result<()> {
    if let Some(reason) = tunnel_route_conflict() {
        return Err(route_conflict_error(reason));
    }
    Ok(())
}

fn route_conflict_error(reason: &'static str) -> crate::domain::errors::BackendError {
    log::warn!("Lumen tunnel route verification failed: {reason}");
    crate::domain::errors::BackendError::new(
        "ROUTING_CONFLICT",
        format!("Windows selected a route outside Lumen: {reason}"),
    )
}

fn tunnel_route_conflict() -> Option<&'static str> {
    use windows_sys::Win32::{
        NetworkManagement::IpHelper::GetBestRoute2, Networking::WinSock::SOCKADDR_INET,
    };
    let Some(luid) = tunnel_luid() else {
        return Some("LumenTun unavailable");
    };
    unsafe {
        for (ip, label) in [
            (std::net::Ipv4Addr::new(1, 1, 1, 1), "cloudflare"),
            (std::net::Ipv4Addr::new(8, 8, 8, 8), "google"),
        ] {
            let mut destination = SOCKADDR_INET::default();
            destination.Ipv4.sin_family = AF_INET;
            destination.Ipv4.sin_addr.S_un.S_addr = u32::from_ne_bytes(ip.octets());
            let mut route = std::mem::zeroed();
            let mut source = SOCKADDR_INET::default();
            if GetBestRoute2(
                std::ptr::null(),
                0,
                std::ptr::null(),
                &destination,
                0,
                &mut route,
                &mut source,
            ) != 0
            {
                return Some(if label == "cloudflare" {
                    "cloudflare lookup"
                } else {
                    "google lookup"
                });
            }
            if route.InterfaceLuid.Value != luid.Value {
                return Some(if label == "cloudflare" {
                    "cloudflare selected another interface"
                } else {
                    "google selected another interface"
                });
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::is_lumen_default_route;

    #[test]
    fn selects_only_lumen_default_routes() {
        let tunnel = 42u64;
        assert!(is_lumen_default_route(tunnel, 0, tunnel));
        assert!(!is_lumen_default_route(tunnel, 24, tunnel));
        assert!(!is_lumen_default_route(tunnel, 32, tunnel));
        assert!(!is_lumen_default_route(7, 0, tunnel));
        assert!(!is_lumen_default_route(7, 128, tunnel));
    }

    #[test]
    fn unreadable_route_table_is_not_treated_as_clear() {
        assert!(super::defaults_absent_from_reads(Some(false), Some(false)));
        assert!(!super::defaults_absent_from_reads(Some(true), Some(false)));
        assert!(!super::defaults_absent_from_reads(Some(false), Some(true)));
        assert!(!super::defaults_absent_from_reads(None, Some(false)));
        assert!(!super::defaults_absent_from_reads(Some(false), None));
        assert!(!super::defaults_absent_from_reads(None, None));
    }
}
