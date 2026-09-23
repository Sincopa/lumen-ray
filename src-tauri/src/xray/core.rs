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
        if ![
            "xray.exe",
            "wintun.dll",
            "LICENSE",
            "geoip.dat",
            "geosite.dat",
        ]
        .contains(&name.as_str())
        {
            return Err(error());
        }
        let dest = directory.join(&name);
        if name == "geoip.dat" || name == "geosite.dat" {
            let geo_src = data.join("geo").join(&name);
            let dest_valid = std::fs::metadata(&dest).is_ok_and(|m| m.len() >= 1024);
            let geo_valid = std::fs::metadata(&geo_src).is_ok_and(|m| m.len() >= 1024);

            if geo_valid {
                let geo_bytes = std::fs::read(&geo_src).map_err(|_| error())?;
                let need_copy =
                    !dest_valid || std::fs::read(&dest).map_or(true, |b| b != geo_bytes);
                if need_copy {
                    crate::xray::updater::atomic_write(&dest, &geo_bytes)?;
                }
            } else if !dest_valid {
                let bytes = std::fs::read(resources.join(&name)).map_err(|_| error())?;
                if format!("{:x}", Sha256::digest(&bytes)) != hash {
                    return Err(error());
                }
                crate::xray::updater::atomic_write(&dest, &bytes)?;
            }
        } else {
            let valid = std::fs::read(&dest)
                .is_ok_and(|bytes| format!("{:x}", Sha256::digest(bytes)) == hash);
            if !valid {
                let bytes = std::fs::read(resources.join(&name)).map_err(|_| error())?;
                if format!("{:x}", Sha256::digest(&bytes)) != hash {
                    return Err(error());
                }
                crate::xray::updater::atomic_write(&dest, &bytes)?;
            }
        }
    }
    if !directory.join("xray.exe").is_file()
        || !directory.join("geoip.dat").is_file()
        || !directory.join("geosite.dat").is_file()
    {
        return Err(error());
    }
    Ok(directory.join("xray.exe"))
}
fn error() -> BackendError {
    BackendError::new(
        "CORE_NOT_FOUND",
        "Verified bundled Xray runtime is missing or invalid",
    )
}
