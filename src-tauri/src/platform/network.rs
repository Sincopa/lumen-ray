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
                let name = if entry.FriendlyName.is_null() {
                    String::new()
                } else {
                    let len = (0..256)
                        .find(|&i| *entry.FriendlyName.add(i) == 0)
                        .unwrap_or(256);
                    String::from_utf16_lossy(std::slice::from_raw_parts(entry.FriendlyName, len))
                };
                if entry.OperStatus == IfOperStatusUp
                    && !name.to_ascii_lowercase().contains("lumentun")
                    && !entry.FirstGatewayAddress.is_null()
                    && !servers.is_empty()
                {
                    let metric = entry.Ipv4Metric;
                    if best.as_ref().is_none_or(|(previous, _)| metric < *previous) {
                        best = Some((metric, servers));
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
    unsafe {
        let name: Vec<u16> = "LumenTun".encode_utf16().chain(Some(0)).collect();
        let mut luid: NET_LUID_LH = std::mem::zeroed();
        if ConvertInterfaceAliasToLuid(name.as_ptr(), &mut luid) != 0 {
            return false;
        }
        let mut table = std::ptr::null_mut();
        if GetIpForwardTable2(AF_INET, &mut table) != 0 || table.is_null() {
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
    unsafe {
        let name: Vec<u16> = "LumenTun".encode_utf16().chain(Some(0)).collect();
        let mut luid: NET_LUID_LH = std::mem::zeroed();
        if ConvertInterfaceAliasToLuid(name.as_ptr(), &mut luid) != 0 {
            return Err(error());
        }
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
