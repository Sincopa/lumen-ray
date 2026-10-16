use crate::{
    domain::errors::{BackendError, Result},
    storage::secure::{OsSecretStore, SecretStore},
};
use sha2::{Digest, Sha256};
use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::RwLock,
};

/// Mixed into the device-id hash so the digest is specific to Lumen.
/// The salt is public. Changing it gives existing installs a new id.
pub const LUMEN_HWID_SALT: &[u8] = b"lumen-ray-device-identity-v1";

/// Cap for the DPAPI identity file. Larger files are rejected.
pub const MAX_IDENTITY_FILE_SIZE: usize = 4096;

/// Accepted length of an `x-hwid` value.
pub const MIN_HWID_LEN: usize = 10;
pub const MAX_HWID_LEN: usize = 64;

fn hwid_error(message: impl Into<String>) -> BackendError {
    BackendError::new("HWID_UNAVAILABLE", message)
}

/// `win-` plus 10 to 64 characters from `[A-Za-z0-9=-]`.
pub fn is_valid_hwid(hwid: &str) -> bool {
    let len = hwid.len();
    if !(MIN_HWID_LEN..=MAX_HWID_LEN).contains(&len) {
        return false;
    }
    if !hwid.starts_with("win-") {
        return false;
    }
    hwid.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '=' || c == '-')
}

/// Short form for logs, for example `win-abcd...1234`.
pub fn mask_hwid(hwid: &str) -> String {
    if hwid.len() <= 12 {
        "win-***".to_string()
    } else {
        format!("{}...{}", &hwid[..8], &hwid[hwid.len() - 4..])
    }
}

/// Printable ASCII, no control characters, at most 256 bytes.
pub fn is_safe_header_value(s: &str) -> bool {
    let trimmed = s.trim();
    !trimmed.is_empty()
        && trimmed.len() <= 256
        && trimmed
            .chars()
            .all(|c| c.is_ascii() && !c.is_ascii_control())
}

/// `win-` and 32 hex chars from the first 16 bytes of SHA-256(salt || seed).
pub fn generate_hwid_from_seed(seed: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(LUMEN_HWID_SALT);
    hasher.update(seed.as_bytes());
    let digest = hasher.finalize();

    let mut hex = String::with_capacity(32);
    for byte in &digest[..16] {
        use std::fmt::Write as _;
        let _ = write!(hex, "{:02x}", byte);
    }
    format!("win-{hex}")
}

/// `HKLM\SOFTWARE\Microsoft\Cryptography\MachineGuid`.
#[cfg(windows)]
pub fn read_machine_seed() -> Option<String> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let key = hklm
        .open_subkey_with_flags(
            r"SOFTWARE\Microsoft\Cryptography",
            KEY_READ | KEY_WOW64_64KEY,
        )
        .or_else(|_| hklm.open_subkey(r"SOFTWARE\Microsoft\Cryptography"))
        .ok()?;

    let guid: String = key.get_value("MachineGuid").ok()?;
    let trimmed = guid.trim();
    if !trimmed.is_empty() {
        Some(trimmed.to_string())
    } else {
        None
    }
}

#[cfg(not(windows))]
pub fn read_machine_seed() -> Option<String> {
    None
}

/// Registry version, for example `10.0.26200`. Falls back to `10.0`.
#[cfg(windows)]
pub fn read_os_version() -> String {
    use winreg::enums::*;
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    if let Ok(key) = hklm.open_subkey_with_flags(
        r"SOFTWARE\Microsoft\Windows NT\CurrentVersion",
        KEY_READ | KEY_WOW64_64KEY,
    ) {
        let major = key
            .get_value::<u32, _>("CurrentMajorVersionNumber")
            .ok()
            .or_else(|| {
                key.get_value::<String, _>("CurrentMajorVersionNumber")
                    .ok()
                    .and_then(|s| s.parse().ok())
            });
        let minor = key
            .get_value::<u32, _>("CurrentMinorVersionNumber")
            .ok()
            .or_else(|| {
                key.get_value::<String, _>("CurrentMinorVersionNumber")
                    .ok()
                    .and_then(|s| s.parse().ok())
            });
        let build = key
            .get_value::<String, _>("CurrentBuildNumber")
            .or_else(|_| key.get_value::<String, _>("CurrentBuild"))
            .ok();

        if let (Some(maj), Some(min), Some(bld)) = (major, minor, build.as_deref()) {
            let ver = format!("{maj}.{min}.{}", bld.trim());
            if is_safe_header_value(&ver) {
                return ver;
            }
        }
    }
    "10.0".to_string()
}

#[cfg(not(windows))]
pub fn read_os_version() -> String {
    "10.0".to_string()
}

/// BIOS product name, when the registry value is usable as a header.
#[cfg(windows)]
pub fn read_device_model() -> Option<String> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let key = hklm
        .open_subkey_with_flags(
            r"HARDWARE\DESCRIPTION\System\BIOS",
            KEY_READ | KEY_WOW64_64KEY,
        )
        .or_else(|_| hklm.open_subkey(r"HARDWARE\DESCRIPTION\System\BIOS"))
        .ok()?;

    let manufacturer: Option<String> = key.get_value("SystemManufacturer").ok();
    let product: Option<String> = key
        .get_value("SystemProductName")
        .or_else(|_| key.get_value("BaseBoardProduct"))
        .ok();

    let clean = |s: String| -> Option<String> {
        let trimmed = s.trim();
        let lower = trimmed.to_ascii_lowercase();
        if trimmed.is_empty()
            || lower.contains("to be filled")
            || lower.contains("system product name")
            || lower.contains("system manufacturer")
            || lower.contains("default string")
            || lower == "none"
            || lower == "oem"
        {
            None
        } else {
            Some(trimmed.to_string())
        }
    };

    let mfg = manufacturer.and_then(clean);
    let prod = product.and_then(clean);

    let model = match (mfg, prod) {
        (Some(m), Some(p)) => {
            if p.to_ascii_lowercase().starts_with(&m.to_ascii_lowercase()) {
                p
            } else {
                format!("{m} {p}")
            }
        }
        (None, Some(p)) => p,
        (Some(m), None) => m,
        (None, None) => return None,
    };

    if is_safe_header_value(&model) {
        Some(model)
    } else {
        None
    }
}

#[cfg(not(windows))]
pub fn read_device_model() -> Option<String> {
    None
}

/// DPAPI file that stores the device id for this Windows user.
pub struct DeviceIdentityStore {
    path: PathBuf,
    secret_store: Box<dyn SecretStore>,
}

impl DeviceIdentityStore {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            secret_store: Box::new(OsSecretStore),
        }
    }

    #[cfg(test)]
    pub fn with_store(path: PathBuf, store: Box<dyn SecretStore>) -> Self {
        Self {
            path,
            secret_store: store,
        }
    }

    /// Attempts to load an existing persisted HWID.
    /// Returns Ok(None) if the identity file does not exist yet.
    /// Returns an error if the file exists but is corrupted, oversized, or fails decryption.
    pub fn load(&self) -> Result<Option<String>> {
        if !self.path.exists() {
            return Ok(None);
        }

        let metadata = std::fs::metadata(&self.path)
            .map_err(|_| hwid_error("Failed to read device identity metadata"))?;

        if metadata.len() > MAX_IDENTITY_FILE_SIZE as u64 || metadata.len() == 0 {
            return Err(hwid_error(
                "Device identity storage is invalid or corrupted",
            ));
        }

        let encrypted = std::fs::read(&self.path)
            .map_err(|_| hwid_error("Failed to read device identity storage"))?;

        if encrypted.len() > MAX_IDENTITY_FILE_SIZE || encrypted.is_empty() {
            return Err(hwid_error(
                "Device identity storage is invalid or corrupted",
            ));
        }

        let decrypted = self
            .secret_store
            .unprotect(&encrypted)
            .map_err(|_| hwid_error("Windows DPAPI failed to decrypt device identity"))?;

        let hwid = String::from_utf8(decrypted)
            .map_err(|_| hwid_error("Decrypted device identity is not valid UTF-8"))?;

        let trimmed = hwid.trim();
        if !is_valid_hwid(trimmed) {
            return Err(hwid_error("Decrypted device identity format is invalid"));
        }

        Ok(Some(trimmed.to_string()))
    }

    /// Atomically persists an HWID to the identity file after encrypting with DPAPI.
    pub fn save(&self, hwid: &str) -> Result<()> {
        if !is_valid_hwid(hwid) {
            return Err(hwid_error("Cannot persist invalid HWID format"));
        }

        let parent = self
            .path
            .parent()
            .ok_or_else(|| hwid_error("Invalid device identity path directory"))?;
        std::fs::create_dir_all(parent)
            .map_err(|_| hwid_error("Failed to create device identity directory"))?;

        let encrypted = self
            .secret_store
            .protect(hwid.as_bytes())
            .map_err(|_| hwid_error("Windows DPAPI failed to protect device identity"))?;

        let mut temp = tempfile::NamedTempFile::new_in(parent)
            .map_err(|_| hwid_error("Failed to create temporary identity file"))?;
        temp.write_all(&encrypted)
            .map_err(|_| hwid_error("Failed to write encrypted device identity"))?;
        temp.as_file()
            .sync_all()
            .map_err(|_| hwid_error("Failed to sync encrypted device identity"))?;
        temp.persist(&self.path)
            .map_err(|_| hwid_error("Failed to persist device identity atomically"))?;

        Ok(())
    }
}

/// Values sent to the subscription host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceIdentity {
    pub hwid: String,
    pub device_os: String,
    pub ver_os: String,
    pub device_model: Option<String>,
}

/// Loads the device id once and keeps it for the process.
pub struct DeviceIdentityManager {
    store: DeviceIdentityStore,
    cached: RwLock<Option<DeviceIdentity>>,
}

impl DeviceIdentityManager {
    pub fn new(path: PathBuf) -> Self {
        Self {
            store: DeviceIdentityStore::new(path),
            cached: RwLock::new(None),
        }
    }

    #[cfg(test)]
    pub fn with_store(path: PathBuf, store: Box<dyn SecretStore>) -> Self {
        Self {
            store: DeviceIdentityStore::with_store(path, store),
            cached: RwLock::new(None),
        }
    }

    /// Returns the cached device identity or loads/creates it once.
    /// If loading or secure persistence fails, returns a typed error and does NOT
    /// silently generate a transient HWID per request.
    pub fn get_or_init(&self) -> Result<DeviceIdentity> {
        if let Ok(guard) = self.cached.read() {
            if let Some(ref identity) = *guard {
                return Ok(identity.clone());
            }
        }

        let mut guard = self
            .cached
            .write()
            .map_err(|_| hwid_error("Device identity lock poisoned"))?;

        // Re-check after acquiring write lock
        if let Some(ref identity) = *guard {
            return Ok(identity.clone());
        }

        let hwid = if let Some(existing) = self.store.load()? {
            log::info!("Loaded existing device HWID: {}", mask_hwid(&existing));
            existing
        } else {
            let seed = read_machine_seed().unwrap_or_else(|| {
                log::warn!("MachineGuid inaccessible; generating random installation seed");
                uuid::Uuid::new_v4().to_string()
            });

            let new_hwid = generate_hwid_from_seed(&seed);
            if !is_valid_hwid(&new_hwid) {
                return Err(hwid_error("Generated HWID failed validation"));
            }

            self.store.save(&new_hwid)?;
            log::info!(
                "Generated and secured new device HWID: {}",
                mask_hwid(&new_hwid)
            );
            new_hwid
        };

        let identity = DeviceIdentity {
            hwid,
            device_os: "Windows".to_string(),
            ver_os: read_os_version(),
            device_model: read_device_model(),
        };

        *guard = Some(identity.clone());
        Ok(identity)
    }

    /// Returns the HWID path on disk.
    pub fn path(&self) -> &Path {
        &self.store.path
    }
}
