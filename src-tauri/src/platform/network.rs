use windows_sys::Win32::{
    NetworkManagement::{
        IpHelper::{ConvertInterfaceAliasToLuid, FreeMibTable, GetIpForwardTable2},
        Ndis::NET_LUID_LH,
    },
    Networking::WinSock::AF_INET,
};
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
        let found = entries.iter().any(|r| {
            r.InterfaceLuid.Value == luid.Value && r.DestinationPrefix.PrefixLength == 0
        });
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
