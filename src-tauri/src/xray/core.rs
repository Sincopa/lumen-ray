use crate::domain::errors::{BackendError, Result};
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
pub fn prepare(resources: &Path, data: &Path) -> Result<PathBuf> {
    // Trust anchor is compiled into the application, never a mutable runtime manifest.
    let manifest: Manifest =
        serde_json::from_str(include_str!("../../resources/core/manifest.json"))
            .map_err(|_| error())?;
    if manifest.architecture != std::env::consts::ARCH || manifest.version != VERSION {
        return Err(error());
    }
    let directory = data.join("core").join(VERSION);
    std::fs::create_dir_all(&directory).map_err(|_| error())?;
    for (name, hash) in manifest.files {
        if !["xray.exe", "wintun.dll", "LICENSE"].contains(&name.as_str()) {
            return Err(error());
        }
        let dest = directory.join(&name);
        let valid =
            std::fs::read(&dest).is_ok_and(|bytes| format!("{:x}", Sha256::digest(bytes)) == hash);
        if !valid {
            let bytes = std::fs::read(resources.join(&name)).map_err(|_| error())?;
            if format!("{:x}", Sha256::digest(&bytes)) != hash {
                return Err(error());
            }
            std::fs::write(&dest, bytes).map_err(|_| error())?;
        }
    }
    Ok(directory.join("xray.exe"))
}
fn error() -> BackendError {
    BackendError::new(
        "CORE_NOT_FOUND",
        "Verified bundled Xray runtime is missing or invalid",
    )
}
