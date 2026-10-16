use crate::domain::errors::{BackendError, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
use winreg::{enums::*, RegKey, RegValue};
const KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Internet Settings";
const NAMES: [&str; 4] = [
    "ProxyEnable",
    "ProxyServer",
    "ProxyOverride",
    "AutoConfigURL",
];
#[derive(Serialize, Deserialize)]
struct StoredValue {
    kind: u32,
    bytes: Vec<u8>,
}
#[derive(Serialize, Deserialize)]
struct Journal {
    previous: BTreeMap<String, Option<StoredValue>>,
    installed: BTreeMap<String, Option<StoredValue>>,
}
pub struct SystemProxy {
    journal: PathBuf,
    port: u16,
}
fn error() -> BackendError {
    BackendError::new(
        "PROXY_FAILED",
        "Could not update or restore Windows proxy settings",
    )
}
fn key() -> Result<RegKey> {
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(KEY, KEY_READ | KEY_WRITE)
        .map_err(|_| error())
}
fn capture(key: &RegKey) -> Result<BTreeMap<String, Option<StoredValue>>> {
    let mut values = BTreeMap::new();
    for name in NAMES {
        let value = match key.get_raw_value(name) {
            Ok(v) => Some(StoredValue {
                kind: v.vtype as u32,
                bytes: v.bytes,
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(_) => return Err(error()),
        };
        values.insert(name.into(), value);
    }
    Ok(values)
}
fn notify() -> Result<()> {
    use windows_sys::Win32::Networking::WinInet::{
        InternetSetOptionW, INTERNET_OPTION_REFRESH, INTERNET_OPTION_SETTINGS_CHANGED,
    };
    unsafe {
        if InternetSetOptionW(
            std::ptr::null_mut(),
            INTERNET_OPTION_SETTINGS_CHANGED,
            std::ptr::null(),
            0,
        ) == 0
            || InternetSetOptionW(
                std::ptr::null_mut(),
                INTERNET_OPTION_REFRESH,
                std::ptr::null(),
                0,
            ) == 0
        {
            return Err(error());
        }
    }
    Ok(())
}
fn string(value: &str) -> StoredValue {
    StoredValue {
        kind: REG_SZ as u32,
        bytes: value
            .encode_utf16()
            .chain(Some(0))
            .flat_map(u16::to_le_bytes)
            .collect(),
    }
}
fn restore_value(key: &RegKey, name: &str, value: &Option<StoredValue>) -> Result<()> {
    if let Some(v) = value {
        let kind = match v.kind {
            1 => REG_SZ,
            2 => REG_EXPAND_SZ,
            3 => REG_BINARY,
            4 => REG_DWORD,
            7 => REG_MULTI_SZ,
            _ => return Err(error()),
        };
        key.set_raw_value(
            name,
            &RegValue {
                vtype: kind,
                bytes: v.bytes.clone(),
            },
        )
        .map_err(|_| error())?;
    } else if let Err(e) = key.delete_value(name) {
        if e.kind() != std::io::ErrorKind::NotFound {
            return Err(error());
        }
    }
    Ok(())
}
impl SystemProxy {
    pub fn enable(directory: &Path, port: u16) -> Result<Self> {
        let journal = directory.join("network-state.json");
        if journal.exists() {
            Self::recover(directory)?;
        }
        let key = key()?;
        let previous = capture(&key)?;
        let mut installed = BTreeMap::new();
        installed.insert(
            "ProxyEnable".into(),
            Some(StoredValue {
                kind: REG_DWORD as u32,
                bytes: 1u32.to_le_bytes().to_vec(),
            }),
        );
        installed.insert(
            "ProxyServer".into(),
            Some(string(&format!(
                "http=127.0.0.1:{port};https=127.0.0.1:{port}"
            ))),
        );
        installed.insert(
            "ProxyOverride".into(),
            Some(string("<local>;localhost;127.*;[::1]")),
        );
        installed.insert("AutoConfigURL".into(), None);
        // Write-ahead journal is protected because existing PAC URLs may contain credentials.
        use crate::storage::secure::{OsSecretStore, SecretStore};
        let data = serde_json::to_vec(&Journal {
            previous,
            installed,
        })
        .map_err(|_| error())?;
        crate::xray::updater::atomic_write(&journal, &OsSecretStore.protect(&data)?)
            .map_err(|_| error())?;
        let this = Self { journal, port };
        let apply = (|| {
            let journal = this.read()?;
            for (name, value) in &journal.installed {
                restore_value(&key, name, value)?;
            }
            notify()
        })();
        if let Err(e) = apply {
            this.restore()?;
            return Err(e);
        }
        Ok(this)
    }
    /// Local HTTP listener the installed Windows proxy points to.
    pub fn port(&self) -> u16 {
        self.port
    }
    fn read(&self) -> Result<Journal> {
        use crate::storage::secure::{OsSecretStore, SecretStore};
        serde_json::from_slice(
            &OsSecretStore.unprotect(&std::fs::read(&self.journal).map_err(|_| error())?)?,
        )
        .map_err(|_| error())
    }
    pub fn restore(&self) -> Result<()> {
        let journal = self.read()?;
        let key = key()?;
        let current = capture(&key)?;
        for name in NAMES {
            let same = match (&current[name], &journal.installed[name]) {
                (None, None) => true,
                (Some(a), Some(b)) => a.kind == b.kind && a.bytes == b.bytes,
                _ => false,
            };
            if same {
                restore_value(&key, name, &journal.previous[name])?;
            }
        }
        notify()?;
        std::fs::remove_file(&self.journal).map_err(|_| error())?;
        Ok(())
    }
    pub fn recover(directory: &Path) -> Result<()> {
        let this = Self {
            journal: directory.join("network-state.json"),
            port: 0,
        };
        if this.journal.exists() {
            this.restore()?;
        }
        Ok(())
    }
}
