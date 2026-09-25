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
                                    AF_INET => Some(std::net::IpAddr::V4(std::net::Ipv4Addr::from(
                                        (*(address.cast::<SOCKADDR_IN>()))
                                            .sin_addr
                                            .S_un
                                            .S_addr
                                            .to_ne_bytes(),
                                    ))),
                                    AF_INET6 => Some(std::net::IpAddr::V6(std::net::Ipv6Addr::from(
                                        (*(address.cast::<SOCKADDR_IN6>())).sin6_addr.u.Byte,
                                    ))),
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
pub fn tunnel_ready() -> bool {
    tunnel_route_exists(AF_INET)
}
fn tunnel_route_exists(family: u16) -> bool {
    let Some(luid) = tunnel_luid() else {
        return false;
    };
    unsafe {
        let mut table = std::ptr::null_mut();
        if GetIpForwardTable2(family, &mut table) != 0 || table.is_null() {
            return false;
        }
        let entries =
            std::slice::from_raw_parts((*table).Table.as_ptr(), (*table).NumEntries as usize);
        let found = entries
            .iter()
            .any(|r| r.InterfaceLuid.Value == luid.Value && r.DestinationPrefix.PrefixLength == 0);
        FreeMibTable(table as _);
        found
    }
}

/// The prepared adapter must not own default routes or outrank ordinary adapters.
pub fn standby_isolated() -> bool {
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        GetIpInterfaceEntry, InitializeIpInterfaceEntry, MIB_IPINTERFACE_ROW,
    };
    if tunnel_route_exists(AF_INET) || tunnel_route_exists(AF_INET6) {
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
        let mut row = MIB_IPINTERFACE_ROW::default();
        InitializeIpInterfaceEntry(&mut row);
        row.Family = AF_INET;
        row.InterfaceLuid = luid;
        GetIpInterfaceEntry(&mut row) == 0 && row.Connected
    }
}

/// Owns only the two default routes created for a prepared TUN session.
/// The helper drops them before it acknowledges a disconnect.
pub struct TunnelRoutes {
    luid: NET_LUID_LH,
    rows: Vec<windows_sys::Win32::NetworkManagement::IpHelper::MIB_IPFORWARD_ROW2>,
    metrics: Vec<(u16, bool, u32)>,
}

impl TunnelRoutes {
    pub fn new() -> crate::domain::errors::Result<Self> {
        use crate::domain::errors::BackendError;
        use windows_sys::Win32::NetworkManagement::IpHelper::{
            GetIpInterfaceEntry, InitializeIpInterfaceEntry, MIB_IPINTERFACE_ROW,
            SetIpInterfaceEntry,
        };
        let luid = tunnel_luid()
            .ok_or_else(|| BackendError::new("TUN_FAILED", "Tunnel adapter is unavailable"))?;
        // Windows also uses interface priority when choosing DNS and application
        // sockets. A prepared adapter must rank behind the physical network.
        for family in [AF_INET, AF_INET6] {
            let mut interface = MIB_IPINTERFACE_ROW::default();
            unsafe { InitializeIpInterfaceEntry(&mut interface); }
            interface.Family = family;
            interface.InterfaceLuid = luid;
            if unsafe { GetIpInterfaceEntry(&mut interface) } != 0 {
                return Err(BackendError::new("TUN_FAILED", "Tunnel interface is not ready"));
            }
            interface.UseAutomaticMetric = false;
            interface.Metric = 5_000;
            if unsafe { SetIpInterfaceEntry(&mut interface) } != 0 {
                return Err(BackendError::new("TUN_FAILED", "Could not lower standby tunnel priority"));
            }
        }
        Ok(Self { luid, rows: Vec::new(), metrics: Vec::new() })
    }

    pub fn activate(&mut self) -> crate::domain::errors::Result<()> {
        use crate::domain::errors::BackendError;
        use windows_sys::Win32::NetworkManagement::IpHelper::{
            CreateIpForwardEntry2, GetIpInterfaceEntry, InitializeIpForwardEntry,
            InitializeIpInterfaceEntry, MIB_IPFORWARD_ROW2, MIB_IPINTERFACE_ROW,
            SetIpInterfaceEntry,
        };
        if self.rows.len() == 2 { return Ok(()); }
        if !self.rows.is_empty() || !self.metrics.is_empty() {
            return Err(BackendError::new("TUN_FAILED", "Tunnel route cleanup is incomplete"));
        }
        for family in [AF_INET, AF_INET6] {
            let mut interface = MIB_IPINTERFACE_ROW::default();
            unsafe { InitializeIpInterfaceEntry(&mut interface); }
            interface.Family = family;
            interface.InterfaceLuid = self.luid;
            if unsafe { GetIpInterfaceEntry(&mut interface) } != 0 {
                let _ = self.deactivate();
                return Err(BackendError::new("TUN_FAILED", "Tunnel interface is not ready"));
            }
            self.metrics.push((family, interface.UseAutomaticMetric, interface.Metric));
            interface.UseAutomaticMetric = false;
            interface.Metric = 0;
            if unsafe { SetIpInterfaceEntry(&mut interface) } != 0 {
                let _ = self.deactivate();
                return Err(BackendError::new("TUN_FAILED", "Could not configure tunnel priority"));
            }
            let mut route = MIB_IPFORWARD_ROW2::default();
            unsafe { InitializeIpForwardEntry(&mut route); }
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
                return Err(BackendError::new("TUN_FAILED", "Could not enable tunnel route"));
            }
            self.rows.push(route);
        }
        if let Err(error) = verify_tunnel_routes() {
            let _ = self.deactivate();
            return Err(error);
        }
        Ok(())
    }

    pub fn deactivate(&mut self) -> crate::domain::errors::Result<()> {
        use crate::domain::errors::BackendError;
        use windows_sys::Win32::Foundation::ERROR_NOT_FOUND;
        use windows_sys::Win32::NetworkManagement::IpHelper::{
            DeleteIpForwardEntry2, GetIpInterfaceEntry, InitializeIpInterfaceEntry,
            MIB_IPINTERFACE_ROW, SetIpInterfaceEntry,
        };
        let mut failed = false;
        let mut remaining_routes = Vec::new();
        for route in self.rows.drain(..) {
            let status = unsafe { DeleteIpForwardEntry2(&route) };
            if status != 0 && status != ERROR_NOT_FOUND {
                failed = true;
                remaining_routes.push(route);
            }
        }
        self.rows = remaining_routes;
        let mut remaining_metrics = Vec::new();
        for (family, automatic, metric) in self.metrics.drain(..) {
            let mut interface = MIB_IPINTERFACE_ROW::default();
            unsafe { InitializeIpInterfaceEntry(&mut interface); }
            interface.Family = family;
            interface.InterfaceLuid = self.luid;
            if unsafe { GetIpInterfaceEntry(&mut interface) } == 0 {
                interface.UseAutomaticMetric = automatic;
                interface.Metric = metric;
                if unsafe { SetIpInterfaceEntry(&mut interface) } != 0 {
                    failed = true;
                    remaining_metrics.push((family, automatic, metric));
                }
            }
        }
        self.metrics = remaining_metrics;
        if failed { Err(BackendError::new("TUN_FAILED", "Could not remove tunnel routes")) }
        else { Ok(()) }
    }
}

/// Check the route Windows would actually select, without binding it to our adapter.
/// A competing VPN can leave our defaults installed but shadow them with split routes.
pub fn verify_tunnel_routes() -> crate::domain::errors::Result<()> {
    use crate::domain::errors::BackendError;
    use windows_sys::Win32::{
        NetworkManagement::IpHelper::GetBestRoute2, Networking::WinSock::SOCKADDR_INET,
    };
    let error = || {
        BackendError::new(
            "ROUTING_CONFLICT",
            "Another network route bypasses Lumen. Disconnect the other VPN and retry",
        )
    };
    let luid = tunnel_luid().ok_or_else(error)?;
    unsafe {
        for ip in [
            std::net::Ipv4Addr::new(1, 1, 1, 1),
            std::net::Ipv4Addr::new(8, 8, 8, 8),
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
                || route.InterfaceLuid.Value != luid.Value
            {
                return Err(error());
            }
        }
    }
    Ok(())
}
