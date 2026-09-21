use crate::domain::errors::{BackendError, Result};
pub trait SecretStore {
    fn protect(&self, input: &[u8]) -> Result<Vec<u8>>;
    fn unprotect(&self, input: &[u8]) -> Result<Vec<u8>>;
}
pub struct OsSecretStore;
#[cfg(windows)]
fn crypt(input: &[u8], encrypt: bool) -> Result<Vec<u8>> {
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::Cryptography::{
            CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
        },
    };
    let data = CRYPT_INTEGER_BLOB {
        cbData: u32::try_from(input.len()).map_err(|_| BackendError::invalid())?,
        pbData: input.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    // DPAPI binds ciphertext to the current Windows user's credentials. No application key is persisted.
    let ok = unsafe {
        if encrypt {
            CryptProtectData(
                &data,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptUnprotectData(
                &data,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        }
    };
    if ok == 0 {
        return Err(BackendError::new(
            "STORAGE_FAILED",
            "Windows could not access protected credentials",
        ));
    }
    let bytes =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    unsafe {
        LocalFree(output.pbData as _);
    }
    Ok(bytes)
}
impl SecretStore for OsSecretStore {
    fn protect(&self, input: &[u8]) -> Result<Vec<u8>> {
        #[cfg(windows)]
        {
            crypt(input, true)
        }
        #[cfg(not(windows))]
        {
            let _ = input;
            Err(BackendError::new(
                "UNSUPPORTED_PLATFORM",
                "Secure storage requires Windows",
            ))
        }
    }
    fn unprotect(&self, input: &[u8]) -> Result<Vec<u8>> {
        #[cfg(windows)]
        {
            crypt(input, false)
        }
        #[cfg(not(windows))]
        {
            let _ = input;
            Err(BackendError::new(
                "UNSUPPORTED_PLATFORM",
                "Secure storage requires Windows",
            ))
        }
    }
}
