//! On-disk Xray executable, licenses and geo files.
//! Starting and stopping the process lives in `crate::engine::xray`.
use crate::{
    domain::errors::{BackendError, Result},
    runtime::{RuntimePaths, XRAY_BINARY},
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

#[derive(Deserialize)]
struct Manifest {
    version: String,
    architecture: String,
    files: BTreeMap<String, String>,
}

pub const VERSION: &str = "26.9.9";

#[cfg(windows)]
const MANIFEST: &str = include_str!("../../runtime/xray/manifest.json");
#[cfg(windows)]
const FILES: &[&str] = &[
    "xray.exe",
    "wintun.dll",
    "LICENSE",
    "xray.txt",
    "geoip.dat",
    "geosite.dat",
];
// One universal binary serves both Apple Silicon and Intel.
#[cfg(not(windows))]
const MANIFEST: &str = include_str!("../../build/manifest-macos.json");
#[cfg(not(windows))]
const FILES: &[&str] = &["xray", "LICENSE", "xray.txt", "geoip.dat", "geosite.dat"];

pub fn prepare(runtime: impl Into<RuntimePaths>, data: &Path) -> Result<PathBuf> {
    let runtime = runtime.into();
    crate::xray::geo::ensure_initialized(&runtime, data)?;
    prepare_files(&runtime, data, false)
}

/// The elevated helper receives a verified Geo snapshot over its authenticated pipe.
/// It only needs the bundled executable and Wintun in its protected staging directory.
#[cfg(windows)]
pub fn prepare_for_tunnel(runtime: impl Into<RuntimePaths>, data: &Path) -> Result<PathBuf> {
    prepare_files(&runtime.into(), data, true)
}

fn prepare_files(runtime: &RuntimePaths, data: &Path, tunnel: bool) -> Result<PathBuf> {
    // Trust anchor is compiled into the application, never a mutable runtime manifest.
    let manifest: Manifest = serde_json::from_str(MANIFEST).map_err(|_| error())?;
    let universal = cfg!(target_os = "macos") && manifest.architecture == "universal";
    if (manifest.architecture != std::env::consts::ARCH && !universal)
        || manifest.version != VERSION
    {
        return Err(error());
    }
    let directory = data.join("core").join(VERSION);
    std::fs::create_dir_all(&directory).map_err(|_| error())?;
    for (name, hash) in manifest.files {
        if !FILES.contains(&name.as_str()) {
            return Err(error());
        }
        if tunnel && matches!(name.as_str(), "geoip.dat" | "geosite.dat") {
            continue;
        }
        let dest = directory.join(&name);
        if matches!(name.as_str(), "geoip.dat" | "geosite.dat") {
            let source = crate::xray::geo::active_dir(data)?.join(&name);
            let bytes = std::fs::read(source).map_err(|_| error())?;
            let valid = std::fs::metadata(&dest).is_ok_and(|m| m.len() == bytes.len() as u64)
                && std::fs::read(&dest).is_ok_and(|existing| existing == bytes);
            if !valid {
                crate::xray::updater::atomic_write(&dest, &bytes)?;
            }
            continue;
        }
        let valid =
            std::fs::read(&dest).is_ok_and(|bytes| format!("{:x}", Sha256::digest(bytes)) == hash);
        if !valid {
            let source = match name.as_str() {
                XRAY_BINARY => runtime.xray_binary(),
                "wintun.dll" => runtime.wintun_path(),
                "xray.txt" | "LICENSE" => runtime.xray_license_path(),
                "geoip.dat" => runtime.geoip_path(),
                "geosite.dat" => runtime.geosite_path(),
                _ => return Err(error()),
            };
            let bytes = std::fs::read(source).map_err(|_| error())?;
            if format!("{:x}", Sha256::digest(&bytes)) != hash {
                return Err(error());
            }
            crate::xray::updater::atomic_write(&dest, &bytes)?;
        }
        // atomic_write creates owner-only files; this also fixes older copies.
        #[cfg(unix)]
        if name == XRAY_BINARY {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755))
                .map_err(|_| error())?;
        }
    }
    if !directory.join(XRAY_BINARY).is_file()
        || (cfg!(windows) && !directory.join("wintun.dll").is_file())
        || (!tunnel
            && (!directory.join("geoip.dat").is_file() || !directory.join("geosite.dat").is_file()))
    {
        return Err(error());
    }
    Ok(directory.join(XRAY_BINARY))
}

fn error() -> BackendError {
    BackendError::new(
        "CORE_NOT_FOUND",
        "Verified bundled Xray runtime is missing or invalid",
    )
}
