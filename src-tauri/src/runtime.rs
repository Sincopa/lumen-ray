use crate::domain::errors::{BackendError, Result};
use std::path::{Path, PathBuf};

fn error() -> BackendError {
    BackendError::new(
        "CORE_NOT_FOUND",
        "Verified bundled Xray runtime is missing or invalid",
    )
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimePaths {
    root: PathBuf,
}

impl RuntimePaths {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn xray_dir(&self) -> PathBuf {
        self.root.join("xray")
    }

    pub fn xray_binary(&self) -> PathBuf {
        self.xray_dir().join("xray.exe")
    }

    pub fn geoip_path(&self) -> PathBuf {
        self.xray_dir().join("geoip.dat")
    }

    pub fn geosite_path(&self) -> PathBuf {
        self.xray_dir().join("geosite.dat")
    }

    pub fn drivers_dir(&self) -> PathBuf {
        self.root.join("drivers")
    }

    pub fn wintun_path(&self) -> PathBuf {
        self.drivers_dir().join("wintun.dll")
    }

    pub fn licenses_dir(&self) -> PathBuf {
        self.root.join("licenses")
    }

    pub fn xray_license_path(&self) -> PathBuf {
        let modern = self.licenses_dir().join("xray.txt");
        if modern.is_file() {
            modern
        } else {
            self.licenses_dir().join("LICENSE")
        }
    }

    pub fn is_valid(&self) -> bool {
        self.xray_binary().is_file() && self.wintun_path().is_file()
    }
}

impl From<PathBuf> for RuntimePaths {
    fn from(root: PathBuf) -> Self {
        Self::new(root)
    }
}

impl From<&PathBuf> for RuntimePaths {
    fn from(root: &PathBuf) -> Self {
        Self::new(root.clone())
    }
}

impl From<&RuntimePaths> for RuntimePaths {
    fn from(paths: &RuntimePaths) -> Self {
        paths.clone()
    }
}

impl From<&Path> for RuntimePaths {
    fn from(root: &Path) -> Self {
        Self::new(root.to_path_buf())
    }
}

/// Resolves runtime paths checking candidates from base dir, env vars, current_exe, and development fallbacks.
pub fn resolve_from(base: Option<&Path>) -> Result<RuntimePaths> {
    let mut candidates = Vec::new();

    // 1. Explicit base directory (e.g. from Tauri resource_dir or test override)
    if let Some(dir) = base {
        if dir.join("xray").join("xray.exe").is_file() {
            candidates.push(dir.to_path_buf());
        }
        let runtime_sub = dir.join("runtime");
        if runtime_sub.join("xray").join("xray.exe").is_file() {
            candidates.push(runtime_sub);
        }
    }

    // 2. Explicit environment variable override
    if let Some(env_dir) = std::env::var_os("LUMEN_RUNTIME_DIR") {
        let path = PathBuf::from(env_dir);
        if path.join("xray").join("xray.exe").is_file() {
            candidates.push(path.clone());
        }
        let sub = path.join("runtime");
        if sub.join("xray").join("xray.exe").is_file() {
            candidates.push(sub);
        }
    }

    // 3. Relative to current executable (works for installed NSIS and portable releases regardless of CWD)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            // Portable or installed app layout: <dir>/runtime
            candidates.push(parent.join("runtime"));
            // Elevated helper or nested layout: <dir>/../runtime
            candidates.push(parent.join("..").join("runtime"));
            candidates.push(parent.join("..").join("..").join("runtime"));
            candidates.push(parent.join("..").join("..").join("..").join("runtime"));
            // Dev layout relative to target/<profile>: target/<profile>/../../src-tauri/runtime
            candidates.push(
                parent
                    .join("..")
                    .join("..")
                    .join("src-tauri")
                    .join("runtime"),
            );
        }
    }

    // 4. Development mode / cargo test fallback via CARGO_MANIFEST_DIR
    if cfg!(debug_assertions) {
        candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("runtime"));
    }

    // Check each candidate and return the first valid runtime directory
    for candidate in candidates {
        let paths = RuntimePaths::new(candidate);
        if paths.is_valid() {
            return Ok(paths);
        }
    }

    Err(error())
}

/// Global resolver for runtime paths without passing an explicit base directory.
pub fn resolve() -> Result<RuntimePaths> {
    resolve_from(None)
}
