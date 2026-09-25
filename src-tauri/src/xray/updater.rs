use crate::domain::errors::{BackendError, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};
const REPOSITORY: &str = "https://api.github.com/repos/XTLS/Xray-core";
const MAX_ARCHIVE: usize = 96 * 1024 * 1024;
#[derive(Deserialize)]
struct Asset {
    name: String,
    digest: Option<String>,
}
#[derive(Deserialize)]
struct Release {
    tag_name: String,
    assets: Vec<Asset>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreUpdate {
    pub version: String,
    pub architecture: String,
    pub sha256: String,
}
fn error() -> BackendError {
    BackendError::new(
        "UPDATE_FAILED",
        "Could not verify or install the Xray update",
    )
}
fn version_parts(version: &str) -> Result<Vec<u32>> {
    if version.len() > 32 {
        return Err(error());
    }
    let values = version
        .split('.')
        .map(|s| s.parse::<u32>().map_err(|_| error()))
        .collect::<Result<Vec<_>>>()?;
    if values.len() != 3 {
        return Err(error());
    }
    Ok(values)
}
fn archive_name() -> Result<&'static str> {
    match std::env::consts::ARCH {
        "x86_64" => Ok("Xray-windows-64.zip"),
        "aarch64" => Ok("Xray-windows-arm64-v8a.zip"),
        _ => Err(error()),
    }
}
fn client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .https_only(true)
        .timeout(Duration::from_secs(90))
        .user_agent("Lumen-core-updater/0.1")
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|_| error())
}
async fn metadata(version: &str) -> Result<CoreUpdate> {
    version_parts(version)?;
    let release: Release = client()?
        .get(format!("{REPOSITORY}/releases/tags/v{version}"))
        .send()
        .await
        .map_err(|_| error())?
        .error_for_status()
        .map_err(|_| error())?
        .json()
        .await
        .map_err(|_| error())?;
    if release.tag_name != format!("v{version}") {
        return Err(error());
    }
    let name = archive_name()?;
    let digest = release
        .assets
        .iter()
        .find(|a| a.name == name)
        .and_then(|a| a.digest.as_deref())
        .and_then(|s| s.strip_prefix("sha256:"))
        .ok_or_else(error)?;
    if digest.len() != 64 || !digest.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err(error());
    }
    Ok(CoreUpdate {
        version: version.into(),
        architecture: std::env::consts::ARCH.into(),
        sha256: digest.into(),
    })
}
pub async fn check(current: &str) -> Result<Option<CoreUpdate>> {
    let releases: Vec<Release> = client()?
        .get(format!("{REPOSITORY}/releases?per_page=10"))
        .send()
        .await
        .map_err(|_| error())?
        .error_for_status()
        .map_err(|_| error())?
        .json()
        .await
        .map_err(|_| error())?;
    let current = version_parts(current)?;
    let newest = releases
        .iter()
        .filter_map(|r| r.tag_name.strip_prefix('v'))
        .filter_map(|v| version_parts(v).ok().map(|p| (p, v)))
        .filter(|(p, _)| p > &current)
        .max_by(|a, b| a.0.cmp(&b.0));
    match newest {
        Some((_, v)) => Ok(Some(metadata(v).await?)),
        None => Ok(None),
    }
}
/// Metadata is fetched from the fixed official HTTPS origin on every activation.
/// Elevated helpers never trust a writable local checksum manifest.
pub async fn prepare(version: &str, directory: &Path) -> Result<PathBuf> {
    let update = metadata(version).await?;
    let archive_name = archive_name()?;
    std::fs::create_dir_all(directory).map_err(|_| error())?;
    let archive_path = directory.join(format!("xray-{version}.zip"));
    let bytes = match std::fs::read(&archive_path) {
        Ok(bytes) if format!("{:x}", Sha256::digest(&bytes)) == update.sha256 => bytes,
        _ => {
            let mut response = client()?
                .get(format!(
                    "https://github.com/XTLS/Xray-core/releases/download/v{version}/{archive_name}"
                ))
                .send()
                .await
                .map_err(|_| error())?
                .error_for_status()
                .map_err(|_| error())?;
            let capacity = response
                .content_length()
                .and_then(|len| usize::try_from(len).ok())
                .map_or(0, |len| len.min(MAX_ARCHIVE));
            let mut bytes = Vec::with_capacity(capacity);
            while let Some(chunk) = response.chunk().await.map_err(|_| error())? {
                if bytes.len() + chunk.len() > MAX_ARCHIVE {
                    return Err(error());
                }
                bytes.extend_from_slice(&chunk);
            }
            if format!("{:x}", Sha256::digest(&bytes)) != update.sha256 {
                return Err(error());
            }
            atomic_write(&archive_path, &bytes)?;
            bytes
        }
    };
    let destination = directory.join(version);
    let staging = tempfile::tempdir_in(directory).map_err(|_| error())?;
    extract(&bytes, staging.path())?;
    let executable = staging.path().join("xray.exe");
    let output = tokio::time::timeout(
        Duration::from_secs(10),
        crate::services::vpn_service::command(&executable)
            .arg("version")
            .output(),
    )
    .await
    .map_err(|_| error())?
    .map_err(|_| error())?;
    if !output.status.success()
        || !String::from_utf8_lossy(&output.stdout).starts_with(&format!("Xray {version} "))
    {
        return Err(error());
    }
    // Ensure staging has wintun.dll (official Xray core zip releases do not bundle it)
    let staging_wintun = staging.path().join("wintun.dll");
    if !staging_wintun.is_file() {
        let base_wintun = directory
            .join(crate::xray::core::VERSION)
            .join("wintun.dll");
        if base_wintun.is_file() {
            let _ = std::fs::copy(&base_wintun, &staging_wintun);
        } else if let Ok(runtime_wintun) = crate::runtime::resolve().map(|r| r.wintun_path()) {
            if runtime_wintun.is_file() {
                let _ = std::fs::copy(&runtime_wintun, &staging_wintun);
            }
        }
    }
    if !staging.path().join("wintun.dll").is_file() {
        return Err(error());
    }
    // Use one committed geo generation, never a mixture of files from the core archive.
    if let Some(data) = directory.parent() {
        if crate::xray::geo::metadata_path(data).exists() {
            let assets = crate::xray::geo::active_dir(data)?;
            for name in ["geoip.dat", "geosite.dat"] {
                std::fs::copy(assets.join(name), staging.path().join(name)).map_err(|_| error())?;
            }
        }
    }
    crate::xray::geo::validate_geo_files_sync(&executable, staging.path())?;
    crate::services::vpn_service::validate(
        &executable,
        br#"{"inbounds":[],"outbounds":[{"protocol":"freedom"}]}"#,
    )
    .await?;
    if destination.exists() {
        // Reuse only byte-identical files; never overwrite an executable that may be running.
        let expected = std::fs::read(staging.path().join("xray.exe")).map_err(|_| error())?;
        if std::fs::read(destination.join("xray.exe")).map_err(|_| error())? != expected {
            return Err(error());
        }
        for name in ["wintun.dll", "geoip.dat", "geosite.dat"] {
            let src = staging.path().join(name);
            let dst = destination.join(name);
            if src.is_file() && !dst.is_file() {
                std::fs::copy(&src, &dst).map_err(|_| error())?;
            }
        }
    } else {
        std::fs::rename(staging.path(), &destination).map_err(|_| error())?;
    }
    Ok(destination.join("xray.exe"))
}
fn extract(bytes: &[u8], destination: &Path) -> Result<()> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|_| error())?;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|_| error())?;
        let enclosed = entry.enclosed_name().ok_or_else(error)?;
        if entry.name().contains('\\') || enclosed.is_absolute() {
            return Err(error());
        }
        if ![
            "xray.exe",
            "wintun.dll",
            "LICENSE",
            "geoip.dat",
            "geosite.dat",
        ]
        .contains(&entry.name())
        {
            continue;
        }
        if entry.size() > 128 * 1024 * 1024 || entry.is_symlink() {
            return Err(error());
        }
        let mut output = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination.join(entry.name()))
            .map_err(|_| error())?;
        std::io::copy(&mut entry.by_ref().take(128 * 1024 * 1024), &mut output)
            .map_err(|_| error())?;
        output.sync_all().map_err(|_| error())?;
    }
    if !destination.join("xray.exe").is_file() {
        return Err(error());
    }
    Ok(())
}
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut temp =
        tempfile::NamedTempFile::new_in(path.parent().ok_or_else(error)?).map_err(|_| error())?;
    temp.write_all(bytes).map_err(|_| error())?;
    temp.as_file().sync_all().map_err(|_| error())?;
    temp.persist(path).map_err(|_| error())?;
    Ok(())
}
pub fn current(directory: &Path) -> String {
    std::fs::read_to_string(directory.join("current-version"))
        .ok()
        .filter(|v| version_parts(v).is_ok())
        .unwrap_or_else(|| super::core::VERSION.into())
}
pub fn activate(directory: &Path, version: &str) -> Result<()> {
    version_parts(version)?;
    let previous = current(directory);
    atomic_write(&directory.join("previous-version"), previous.as_bytes())?;
    atomic_write(&directory.join("current-version"), version.as_bytes())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    #[ignore = "Downloads a verified official release; requires Internet access"]
    async fn official_release_integrity_and_activation() {
        let directory = tempfile::tempdir().unwrap();
        let version = super::super::core::VERSION;
        let update = metadata(version).await.unwrap();
        assert_eq!(update.architecture, std::env::consts::ARCH);
        assert_eq!(update.sha256.len(), 64);
        let executable = prepare(version, directory.path()).await.unwrap();
        assert!(executable.is_file());
        activate(directory.path(), version).unwrap();
        assert_eq!(current(directory.path()), version);
        // A modified local executable must never be trusted just because metadata is cached.
        std::fs::write(&executable, b"tampered").unwrap();
        assert!(prepare(version, directory.path()).await.is_err());
    }
    #[test]
    fn rejects_version_paths() {
        for value in ["../1.2", "1/2/3", "https://example.com", "1.2", "1.2.3.4"] {
            assert!(version_parts(value).is_err());
        }
    }
    #[test]
    fn prevents_archive_traversal() {
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        writer
            .start_file("../xray.exe", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"bad").unwrap();
        let bytes = writer.finish().unwrap().into_inner();
        let dir = tempfile::tempdir().unwrap();
        assert!(extract(&bytes, dir.path()).is_err());
    }
}
