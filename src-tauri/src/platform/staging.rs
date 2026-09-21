//! Elevated core staging must not be writable by the unelevated desktop user.
use crate::domain::errors::{BackendError, Result};
use std::{
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
};
use windows_sys::Win32::{
    Foundation::LocalFree,
    Security::{
        Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW, SECURITY_ATTRIBUTES,
    },
    Storage::FileSystem::CreateDirectoryW,
    System::SystemInformation::GetWindowsDirectoryW,
};
pub struct Directory(PathBuf);
impl Directory {
    pub fn create() -> Result<Self> {
        let error = || {
            BackendError::new(
                "PERMISSION_DENIED",
                "Cannot create protected tunnel runtime directory",
            )
        };
        unsafe {
            let mut windows = vec![0u16; 32768];
            let length = GetWindowsDirectoryW(windows.as_mut_ptr(), windows.len() as u32) as usize;
            if length == 0 || length >= windows.len() {
                return Err(error());
            }
            let path = PathBuf::from(String::from_utf16_lossy(&windows[..length]))
                .join("Temp")
                .join(format!("LumenTunnel-{}", uuid::Uuid::new_v4()));
            // Protected ACL; OWNER RIGHTS prevents user-owned descendants from implicitly
            // granting WRITE_DAC to a medium-integrity token. Administrators retain full access.
            let acl: Vec<u16> = "O:BAG:BAD:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;RC;;;OW)"
                .encode_utf16()
                .chain(Some(0))
                .collect();
            let mut descriptor = std::ptr::null_mut();
            if ConvertStringSecurityDescriptorToSecurityDescriptorW(
                acl.as_ptr(),
                1,
                &mut descriptor,
                std::ptr::null_mut(),
            ) == 0
            {
                return Err(error());
            }
            let security = SECURITY_ATTRIBUTES {
                nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: descriptor,
                bInheritHandle: 0,
            };
            let name: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
            let created = CreateDirectoryW(name.as_ptr(), &security);
            LocalFree(descriptor);
            if created == 0 {
                return Err(error());
            }
            Ok(Self(path))
        }
    }
    pub fn path(&self) -> &Path {
        &self.0
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if std::fs::remove_dir_all(&self.0).is_err() {
            log::warn!("Protected core staging cleanup failed");
        }
    }
}
