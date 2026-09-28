//! IP Helper notifications with no borrowed callback state or logged adapter identities.
use std::sync::atomic::{AtomicU64, Ordering};
use windows_sys::Win32::{
    Foundation::HANDLE,
    NetworkManagement::IpHelper::{
        CancelMibChangeNotify2, NotifyIpInterfaceChange, MIB_IPINTERFACE_ROW, MIB_NOTIFICATION_TYPE,
    },
    Networking::WinSock::AF_UNSPEC,
};
static REVISION: AtomicU64 = AtomicU64::new(0);

/// Only physical adapter addresses are fingerprinted; Lumen's own route events
/// must not trigger a restart loop. The fingerprint never leaves memory.
pub fn physical_fingerprint() -> Option<u64> {
    use std::hash::{Hash, Hasher};
    use windows_sys::Win32::Foundation::ERROR_BUFFER_OVERFLOW;
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        GetAdaptersAddresses, GAA_FLAG_INCLUDE_GATEWAYS, IP_ADAPTER_ADDRESSES_LH,
    };
    use windows_sys::Win32::NetworkManagement::Ndis::IfOperStatusUp;
    let mut size = 16384;
    for _ in 0..3 {
        let mut buffer = vec![0u64; (size as usize).div_ceil(8)];
        size = (buffer.len() * 8) as u32;
        let status = unsafe {
            GetAdaptersAddresses(
                AF_UNSPEC as u32,
                GAA_FLAG_INCLUDE_GATEWAYS,
                std::ptr::null(),
                buffer.as_mut_ptr().cast(),
                &mut size,
            )
        };
        if status == ERROR_BUFFER_OVERFLOW {
            continue;
        }
        if status != 0 {
            return None;
        }
        let mut adapters = Vec::new();
        let mut entry = buffer.as_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>();
        unsafe {
            while let Some(adapter) = entry.as_ref() {
                if matches!(adapter.IfType, 6 | 71)
                    && adapter.OperStatus == IfOperStatusUp
                    && !adapter.FirstGatewayAddress.is_null()
                {
                    let mut addresses = Vec::new();
                    let mut address = adapter.FirstUnicastAddress;
                    while let Some(item) = address.as_ref() {
                        let socket = item.Address.lpSockaddr;
                        if !socket.is_null() {
                            use windows_sys::Win32::Networking::WinSock::{
                                AF_INET, AF_INET6, SOCKADDR_IN, SOCKADDR_IN6,
                            };
                            match (*socket).sa_family {
                                AF_INET
                                    if item.Address.iSockaddrLength as usize
                                        >= std::mem::size_of::<SOCKADDR_IN>() =>
                                {
                                    addresses.push((
                                        0,
                                        (*(socket.cast::<SOCKADDR_IN>()))
                                            .sin_addr
                                            .S_un
                                            .S_addr
                                            .to_ne_bytes()
                                            .to_vec(),
                                    ));
                                }
                                AF_INET6
                                    if item.Address.iSockaddrLength as usize
                                        >= std::mem::size_of::<SOCKADDR_IN6>() =>
                                {
                                    addresses.push((
                                        1,
                                        (*(socket.cast::<SOCKADDR_IN6>()))
                                            .sin6_addr
                                            .u
                                            .Byte
                                            .to_vec(),
                                    ));
                                }
                                _ => {}
                            }
                        }
                        address = item.Next;
                    }
                    addresses.sort();
                    adapters.push((adapter.Luid.Value, addresses));
                }
                entry = adapter.Next;
            }
        }
        adapters.sort();
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        adapters.hash(&mut hasher);
        // Include DHCP DNS changes even if the adapter address stays the same.
        crate::platform::network::system_dns()
            .ok()
            .hash(&mut hasher);
        return Some(hasher.finish());
    }
    None
}
unsafe extern "system" fn changed(
    _: *const core::ffi::c_void,
    _: *const MIB_IPINTERFACE_ROW,
    _: MIB_NOTIFICATION_TYPE,
) {
    REVISION.fetch_add(1, Ordering::Relaxed);
}
pub struct NetworkWatch {
    handle: HANDLE,
}
unsafe impl Send for NetworkWatch {}
impl NetworkWatch {
    pub fn new() -> Self {
        let mut handle = std::ptr::null_mut();
        if unsafe {
            NotifyIpInterfaceChange(
                AF_UNSPEC,
                Some(changed),
                std::ptr::null(),
                false,
                &mut handle,
            )
        } != 0
        {
            log::warn!("Network event registration failed; periodic health checks remain enabled");
        }
        Self { handle }
    }
    pub fn revision(&self) -> u64 {
        REVISION.load(Ordering::Relaxed)
    }
}
impl Drop for NetworkWatch {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                CancelMibChangeNotify2(self.handle);
            }
        }
    }
}
