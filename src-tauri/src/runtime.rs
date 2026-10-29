use crate::domain::errors::{BackendError, Result};
use std::path::{Path, PathBuf};

#[cfg(windows)]
pub const XRAY_BINARY: &str = "xray.exe";
#[cfg(not(windows))]
pub const XRAY_BINARY: &str = "xray";

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
        self.xray_dir().join(XRAY_BINARY)
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
        // Wintun only backs the Windows TUN helper.
        has_runtime(&self.root) && (!cfg!(windows) || self.wintun_path().is_file())
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

/// Android embeds Xray in the VPN service; its runtime is the geo databases
/// RuntimeProvider copies out of the APK (LUMEN_RUNTIME_DIR).
fn has_runtime(root: &Path) -> bool {
    if cfg!(target_os = "android") {
        root.join("xray").join("geosite.dat").is_file()
    } else {
        root.join("xray").join(XRAY_BINARY).is_file()
    }
}

/// Resolves runtime paths checking candidates from base dir, env vars, current_exe, and development fallbacks.
pub fn resolve_from(base: Option<&Path>) -> Result<RuntimePaths> {
    let mut candidates = Vec::new();

    // 1. Explicit base directory (e.g. from Tauri resource_dir or test override)
    if let Some(dir) = base {
        if has_runtime(dir) {
            candidates.push(dir.to_path_buf());
        }
        let runtime_sub = dir.join("runtime");
        if has_runtime(&runtime_sub) {
            candidates.push(runtime_sub);
        }
    }

    // 2. Explicit environment variable override
    if let Some(env_dir) = std::env::var_os("LUMEN_RUNTIME_DIR") {
        let path = PathBuf::from(env_dir);
        if has_runtime(&path) {
            candidates.push(path.clone());
        }
        let sub = path.join("runtime");
        if has_runtime(&sub) {
            candidates.push(sub);
        }
    }

    // 3. Relative to current executable (works for installed NSIS and portable releases regardless of CWD)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            // macOS app bundle: Contents/MacOS/<exe> -> Contents/Resources/runtime
            #[cfg(target_os = "macos")]
            candidates.push(parent.join("..").join("Resources").join("runtime"));
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
