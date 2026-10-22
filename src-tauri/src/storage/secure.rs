use crate::domain::errors::{BackendError, Result};
pub trait SecretStore: Send + Sync {
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
/// AES-256-GCM with a random key in the login Keychain. Blob: version, nonce, ciphertext.
#[cfg(target_os = "macos")]
mod keychain {
    use super::{BackendError, Result};
    use aes_gcm::{
        aead::{Aead, Payload},
        Aes256Gcm, KeyInit, Nonce,
    };
    use zeroize::Zeroizing;
    const VERSION: u8 = 1;
    const NONCE: usize = 12;
    static KEY: std::sync::Mutex<Option<Zeroizing<[u8; 32]>>> = std::sync::Mutex::new(None);
    fn failed(message: &str) -> BackendError {
        BackendError::new("STORAGE_FAILED", message)
    }
    fn random(buffer: &mut [u8]) -> Result<()> {
        security_framework::random::SecRandom::default()
            .copy_bytes(buffer)
            .map_err(|_| failed("macOS could not generate random data"))
    }
    /// Only sealing creates a key, so a reset keychain cannot replace the key of existing data.
    #[cfg(not(test))]
    fn load(create: bool) -> Result<Zeroizing<[u8; 32]>> {
        use security_framework::passwords::{get_generic_password, set_generic_password};
        const SERVICE: &str = "app.lumen.desktop";
        const ACCOUNT: &str = "storage-key";
        const NOT_FOUND: i32 = -25300; // errSecItemNotFound
        let mut key = Zeroizing::new([0u8; 32]);
        match get_generic_password(SERVICE, ACCOUNT) {
            Ok(stored) => {
                let stored = Zeroizing::new(stored);
                if stored.len() != key.len() {
                    return Err(failed("The Lumen key in the macOS Keychain is damaged"));
                }
                key.copy_from_slice(&stored);
            }
            Err(error) if error.code() == NOT_FOUND && !create => {
                return Err(failed("The Lumen key is missing from the macOS Keychain"));
            }
            Err(error) if error.code() == NOT_FOUND => {
                random(key.as_mut())?;
                set_generic_password(SERVICE, ACCOUNT, key.as_ref())
                    .map_err(|_| failed("macOS Keychain could not store the Lumen key"))?;
            }
            Err(_) => return Err(failed("macOS Keychain could not provide the Lumen key")),
        }
        Ok(key)
    }
    #[cfg(test)]
    fn load(_create: bool) -> Result<Zeroizing<[u8; 32]>> {
        let mut key = Zeroizing::new([0u8; 32]);
        random(key.as_mut())?;
        Ok(key)
    }
    fn cipher(create: bool) -> Result<Aes256Gcm> {
        // One lock: concurrent first uses must not each create a different key.
        let mut cached = KEY
            .lock()
            .map_err(|_| failed("macOS Keychain could not provide the Lumen key"))?;
        if cached.is_none() {
            *cached = Some(load(create)?);
        }
        let key = cached.as_ref().ok_or_else(BackendError::invalid)?;
        Aes256Gcm::new_from_slice(key.as_ref()).map_err(|_| BackendError::invalid())
    }
    pub fn seal(input: &[u8]) -> Result<Vec<u8>> {
        let mut nonce = [0u8; NONCE];
        random(&mut nonce)?;
        let sealed = cipher(true)?
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: input,
                    aad: &[VERSION],
                },
            )
            .map_err(|_| failed("macOS could not protect Lumen data"))?;
        let mut output = Vec::with_capacity(1 + NONCE + sealed.len());
        output.push(VERSION);
        output.extend_from_slice(&nonce);
        output.extend_from_slice(&sealed);
        Ok(output)
    }
    pub fn open(input: &[u8]) -> Result<Vec<u8>> {
        let damaged = || failed("Protected Lumen data is damaged or belongs to another macOS user");
        if input.len() < 1 + NONCE + 16 || input[0] != VERSION {
            return Err(damaged());
        }
        cipher(false)?
            .decrypt(
                Nonce::from_slice(&input[1..1 + NONCE]),
                Payload {
                    msg: &input[1 + NONCE..],
                    aad: &[VERSION],
                },
            )
            .map_err(|_| damaged())
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn sealed_blobs_round_trip_and_reject_tampering() {
            let sealed = super::seal(b"secret").unwrap();
            assert_eq!(sealed[0], 1);
            assert_eq!(sealed.len(), 1 + 12 + 6 + 16);
            assert_eq!(super::open(&sealed).unwrap(), b"secret");
            assert_ne!(super::seal(b"secret").unwrap(), sealed);
            for index in [0, 5, sealed.len() - 1] {
                let mut bad = sealed.clone();
                bad[index] ^= 1;
                assert_eq!(super::open(&bad).unwrap_err().code, "STORAGE_FAILED");
            }
            assert!(super::open(&sealed[..20]).is_err());
            assert_eq!(super::open(&super::seal(b"").unwrap()).unwrap(), b"");
        }
    }
}
impl SecretStore for OsSecretStore {
    fn protect(&self, input: &[u8]) -> Result<Vec<u8>> {
        #[cfg(windows)]
        {
            crypt(input, true)
        }
        #[cfg(target_os = "macos")]
        {
            keychain::seal(input)
        }
        #[cfg(not(any(windows, target_os = "macos")))]
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
        #[cfg(target_os = "macos")]
        {
            keychain::open(input)
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        {
            let _ = input;
            Err(BackendError::new(
                "UNSUPPORTED_PLATFORM",
                "Secure storage requires Windows",
            ))
        }
    }
}
