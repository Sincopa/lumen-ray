use crate::domain::errors::{BackendError, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

pub const GEO_REPOSITORY: &str = "https://api.github.com/repos/NetworkKeeper/v2ray-rules-ru";
pub const GEO_SOURCE_NAME: &str = "NetworkKeeper/v2ray-rules-ru";
pub const BUNDLED_GEO_VERSION: &str = "202410152325";
pub const MAX_GEOIP_SIZE: usize = 32 * 1024 * 1024;
pub const MAX_GEOSITE_SIZE: usize = 64 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeoDatabaseMetadata {
    pub version: String,
    pub source: String,
    pub updated_at: u64,
    pub geoip_sha256: String,
    pub geosite_sha256: String,
    #[serde(default)]
    pub generation: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeoDatabaseInfo {
    pub version: String,
    pub source: String,
    pub updated_at: u64,
    pub status: String,
    pub error: Option<String>,
    pub valid: bool,
    pub geoip_size: u64,
    pub geosite_size: u64,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    size: usize,
    digest: Option<String>,
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    assets: Vec<Asset>,
}

fn error() -> BackendError {
    BackendError::new(
        "GEO_UPDATE_FAILED",
        "Could not verify or install GeoIP/GeoSite database update",
    )
}

fn client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .https_only(true)
        .timeout(Duration::from_secs(90))
        .user_agent("Lumen-geo-updater/0.1")
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|_| error())
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub fn geo_dir(data: &Path) -> PathBuf {
    data.join("geo")
}

pub fn metadata_path(data: &Path) -> PathBuf {
    geo_dir(data).join("metadata.json")
}

// Each committed metadata file points to an immutable, fully validated pair.
// A crash before the single metadata rename leaves the previous generation active.
fn metadata(data: &Path) -> Result<GeoDatabaseMetadata> {
    serde_json::from_slice(&std::fs::read(metadata_path(data)).map_err(|_| error())?)
        .map_err(|_| error())
}
pub fn active_dir(data: &Path) -> Result<PathBuf> {
    let meta = metadata(data)?;
    match meta.generation {
        Some(id) if uuid::Uuid::parse_str(&id).is_ok() => Ok(geo_dir(data).join(id)),
        None => Ok(geo_dir(data)), // Read legacy installations without changing their rules.
        _ => Err(error()),
    }
}
pub fn assets_for_core(core: &Path) -> PathBuf {
    let parent = core.parent().unwrap_or(Path::new("."));
    if let Some(data) = parent.parent().and_then(Path::parent) {
        if metadata_path(data).is_file() {
            // Invalid metadata must fail closed, never silently load older bundled files.
            return active_dir(data).unwrap_or_else(|_| geo_dir(data).join("invalid"));
        }
    }
    parent.to_path_buf()
}
fn verify_pair(dir: &Path, meta: &GeoDatabaseMetadata) -> Result<(Vec<u8>, Vec<u8>)> {
    let ip = std::fs::read(dir.join("geoip.dat")).map_err(|_| error())?;
    let site = std::fs::read(dir.join("geosite.dat")).map_err(|_| error())?;
    if ip.len() < 1024 || ip.len() > MAX_GEOIP_SIZE || site.len() < 1024 || site.len() > MAX_GEOSITE_SIZE
        || format!("{:x}", Sha256::digest(&ip)) != meta.geoip_sha256
        || format!("{:x}", Sha256::digest(&site)) != meta.geosite_sha256 {
        return Err(error());
    }
    Ok((ip, site))
}
pub fn info(data: &Path) -> GeoDatabaseInfo {
    let verified = snapshot(data).map(|(m, ip, site)| (m, ip.len() as u64, site.len() as u64));
    match verified {
        Ok((m, ip, site)) => GeoDatabaseInfo {
            version:m.version, source:m.source, updated_at:m.updated_at,
            status:"ready".into(), error:None, valid:true, geoip_size:ip, geosite_size:site,
        },
        Err(_) => GeoDatabaseInfo {
            version:"none".into(), source:GEO_SOURCE_NAME.into(), updated_at:0,
            status:if metadata_path(data).exists() { "corrupted" } else { "missing" }.into(),
            error:Some("Geo databases are missing or have not passed validation".into()),
            valid:false, geoip_size:0, geosite_size:0,
        },
    }
}
fn lock(data: &Path) -> Result<std::fs::File> {
    std::fs::create_dir_all(geo_dir(data)).map_err(|_| error())?;
    let file = std::fs::OpenOptions::new().create(true).truncate(false).read(true).write(true)
        .open(geo_dir(data).join("update.lock")).map_err(|_| error())?;
    fs2::FileExt::try_lock_exclusive(&file).map_err(|_| BackendError::new("GEO_UPDATE_BUSY", "A geo database update is already running"))?;
    Ok(file)
}
fn commit(data: &Path, staging: tempfile::TempDir, mut meta: GeoDatabaseMetadata) -> Result<()> {
    let id = uuid::Uuid::new_v4().to_string();
    let destination = geo_dir(data).join(&id);
    std::fs::rename(staging.path(), &destination).map_err(|_| error())?;
    meta.generation = Some(id);
    let bytes = serde_json::to_vec(&meta).map_err(|_| error())?;
    crate::xray::updater::atomic_write(&metadata_path(data), &bytes)
}
pub fn restore_metadata(data: &Path, bytes: &[u8]) -> Result<()> {
    let _lock = lock(data)?;
    let meta: GeoDatabaseMetadata = serde_json::from_slice(bytes).map_err(|_| error())?;
    let id = meta.generation.as_ref().ok_or_else(error)?;
    if uuid::Uuid::parse_str(id).is_err() { return Err(error()); }
    verify_pair(&geo_dir(data).join(id), &meta)?;
    crate::xray::updater::atomic_write(&metadata_path(data), bytes)
}
pub fn snapshot(data: &Path) -> Result<(GeoDatabaseMetadata, Vec<u8>, Vec<u8>)> {
    let meta = metadata(data)?;
    let dir = match &meta.generation {
        Some(id) if uuid::Uuid::parse_str(id).is_ok() => geo_dir(data).join(id),
        _ => return Err(error()),
    };
    let (ip, site) = verify_pair(&dir, &meta)?;
    Ok((meta, ip, site))
}
#[allow(dead_code)]
pub fn install_snapshot(data: &Path, core: &Path, meta: GeoDatabaseMetadata, ip: &[u8], site: &[u8]) -> Result<()> {
    let _lock = lock(data)?;
    let staging = tempfile::tempdir_in(geo_dir(data)).map_err(|_| error())?;
    crate::xray::updater::atomic_write(&staging.path().join("geoip.dat"), ip)?;
    crate::xray::updater::atomic_write(&staging.path().join("geosite.dat"), site)?;
    verify_pair(staging.path(), &meta)?;
    validate_geo_files_sync(core, staging.path())?;
    commit(data, staging, meta)
}
pub fn install_trusted_snapshot(data: &Path, meta: GeoDatabaseMetadata, ip: &[u8], site: &[u8]) -> Result<()> {
    let _lock = lock(data)?;
    let staging = tempfile::tempdir_in(geo_dir(data)).map_err(|_| error())?;
    crate::xray::updater::atomic_write(&staging.path().join("geoip.dat"), ip)?;
    crate::xray::updater::atomic_write(&staging.path().join("geosite.dat"), site)?;
    verify_pair(staging.path(), &meta)?;
    commit(data, staging, meta)
}
pub fn ensure_initialized(runtime: impl Into<crate::runtime::RuntimePaths>, data: &Path) -> Result<()> {
    let runtime = runtime.into();
    let _lock = lock(data)?;
    if info(data).valid { return Ok(()); }
    let core = runtime.xray_binary();
    let staging = tempfile::tempdir_in(geo_dir(data)).map_err(|_| error())?;
    let legacy = metadata(data).ok().and_then(|m| {
        let pair = verify_pair(&active_dir(data).ok()?, &m).ok()?;
        Some((m, pair))
    });
    let (meta, (ip, site)) = if let Some(legacy) = legacy { legacy } else {
        let manifest: serde_json::Value = serde_json::from_str(include_str!("../../runtime/xray/manifest.json")).map_err(|_| error())?;
        let meta = GeoDatabaseMetadata {
            version:BUNDLED_GEO_VERSION.into(), source:GEO_SOURCE_NAME.into(), updated_at:now_ms(),
            geoip_sha256:manifest["files"]["geoip.dat"].as_str().ok_or_else(error)?.into(),
            geosite_sha256:manifest["files"]["geosite.dat"].as_str().ok_or_else(error)?.into(), generation:None,
        };
        let pair = verify_pair(&runtime.xray_dir(), &meta)?;
        (meta, pair)
    };
    crate::xray::updater::atomic_write(&staging.path().join("geoip.dat"), &ip)?;
    crate::xray::updater::atomic_write(&staging.path().join("geosite.dat"), &site)?;
    validate_geo_files_sync(&core, staging.path())?;
    commit(data, staging, meta)
}

pub async fn update(data: &Path, core_path: &Path) -> Result<GeoDatabaseInfo> {
    let _lock = lock(data)?;
    if !core_path.is_file() { return Err(error()); }
    let release_url = format!("{GEO_REPOSITORY}/releases/latest");
    let release: Release = client()?
        .get(&release_url)
        .send()
        .await
        .map_err(|_| error())?
        .error_for_status()
        .map_err(|_| error())?
        .json()
        .await
        .map_err(|_| error())?;

    let geoip_asset = release
        .assets
        .iter()
        .find(|a| a.name == "geoip.dat")
        .ok_or_else(error)?;
    let geosite_asset = release
        .assets
        .iter()
        .find(|a| a.name == "geosite.dat")
        .ok_or_else(error)?;

    if geoip_asset.size > MAX_GEOIP_SIZE || geosite_asset.size > MAX_GEOSITE_SIZE {
        return Err(BackendError::new(
            "GEO_UPDATE_FAILED",
            "Downloaded database size exceeds security limit",
        ));
    }

    let dir = geo_dir(data);
    std::fs::create_dir_all(&dir).map_err(|_| error())?;
    let staging = tempfile::tempdir_in(&dir).map_err(|_| error())?;

    // Download geoip.dat
    let geoip_bytes = download_file(&geoip_asset.browser_download_url, MAX_GEOIP_SIZE).await?;
    let geoip_hash = format!("{:x}", Sha256::digest(&geoip_bytes));
    let staging_geoip = staging.path().join("geoip.dat");
    std::fs::write(&staging_geoip, &geoip_bytes).map_err(|_| error())?;

    // Download geosite.dat
    let geosite_bytes =
        download_file(&geosite_asset.browser_download_url, MAX_GEOSITE_SIZE).await?;
    let geosite_hash = format!("{:x}", Sha256::digest(&geosite_bytes));
    let staging_geosite = staging.path().join("geosite.dat");
    std::fs::write(&staging_geosite, &geosite_bytes).map_err(|_| error())?;

    verify_download(geoip_asset, &geoip_hash)?;
    verify_download(geosite_asset, &geosite_hash)?;
    validate_geo_files_sync(core_path, staging.path())?;

    let meta = GeoDatabaseMetadata {
        version:release.tag_name, source:GEO_SOURCE_NAME.into(), updated_at:now_ms(),
        geoip_sha256:geoip_hash, geosite_sha256:geosite_hash, generation:None,
    };
    commit(data, staging, meta)?;
    Ok(info(data))
}

// Old NetworkKeeper releases predate GitHub asset digests. Only the exact bundled
// release is accepted without a published digest, using the compiled trust anchor.
fn verify_download(asset: &Asset, actual: &str) -> Result<()> {
    let manifest: serde_json::Value = serde_json::from_str(include_str!("../../runtime/xray/manifest.json")).map_err(|_| error())?;
    let expected = asset.digest.as_deref().and_then(|s| s.strip_prefix("sha256:"))
        .or_else(|| manifest["files"][&asset.name].as_str()).ok_or_else(error)?;
    if expected != actual { return Err(error()); }
    Ok(())
}

async fn download_file(url: &str, max_size: usize) -> Result<Vec<u8>> {
    let mut response = client()?
        .get(url)
        .send()
        .await
        .map_err(|_| error())?
        .error_for_status()
        .map_err(|_| error())?;

    let capacity = response
        .content_length()
        .and_then(|len| usize::try_from(len).ok())
        .map_or(0, |len| len.min(max_size));
    let mut bytes = Vec::with_capacity(capacity);
    while let Some(chunk) = response.chunk().await.map_err(|_| error())? {
        if bytes.len() + chunk.len() > max_size {
            return Err(BackendError::new(
                "GEO_UPDATE_FAILED",
                "File size exceeds permitted maximum",
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    if bytes.is_empty() {
        return Err(error());
    }
    Ok(bytes)
}

pub fn validate_geo_files_sync(core_path: &Path, asset_dir: &Path) -> Result<()> {
    // Concurrent first-run preparations otherwise spawn one Go worker pool per
    // database check and can starve each other until their deadlines expire.
    static VALIDATION: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _validation = VALIDATION.lock().map_err(|_| error())?;
    let test_config = serde_json::json!({
        "log": { "loglevel": "none" },
        "inbounds": [{ "port": std::net::TcpListener::bind("127.0.0.1:0").ok().and_then(|l| l.local_addr().ok()).map(|a| a.port()).unwrap_or(10808), "protocol": "socks" }],
        "outbounds": [{ "tag": "direct", "protocol": "freedom" }],
        "routing": {
            "domainStrategy": "IPIfNonMatch",
            "rules": [
                { "type": "field", "ip": ["geoip:private"], "outboundTag": "direct" },
                { "type": "field", "domain": ["geosite:youtube", "geosite:antifilter"], "outboundTag": "direct" },
                { "type": "field", "domain": ["geosite:antifilter-community"], "outboundTag": "direct" },
                { "type": "field", "ip": ["geoip:antifilter"], "outboundTag": "direct" },
                { "type": "field", "ip": ["geoip:antifilter-community"], "outboundTag": "direct" },
                { "type": "field", "domain": ["geosite:category-ru"], "outboundTag": "direct" },
                { "type": "field", "ip": ["geoip:ru"], "outboundTag": "direct" }
            ]
        }
    });

    let config_bytes = serde_json::to_vec(&test_config).map_err(|_| error())?;
    let mut cmd = std::process::Command::new(core_path);
    #[cfg(windows)] {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    cmd.args(["run", "-test", "-config", "stdin:"])
        .env("GOMAXPROCS", "2")
        .env("XRAY_LOCATION_ASSET", asset_dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    let mut child = cmd.spawn().map_err(|_| error())?;
    use std::io::Write;
    let sent = child.stdin.take().ok_or_else(error)?.write_all(&config_bytes);
    if sent.is_err() { let _ = child.kill(); let _ = child.wait(); return Err(error()); }
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return if status.success() { Ok(()) } else {
                Err(BackendError::new("GEO_UPDATE_FAILED", "Xray rejected the geo database validation configuration"))
            },
            Ok(None) if std::time::Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            Ok(None) => { let _ = child.kill(); let _ = child.wait(); return Err(BackendError::new("GEO_UPDATE_FAILED", "Xray geo database validation timed out")); }
            Err(_) => { let _ = child.kill(); let _ = child.wait(); return Err(BackendError::new("GEO_UPDATE_FAILED", "Cannot inspect Xray geo validation process")); }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn runtime() -> crate::runtime::RuntimePaths {
        crate::runtime::RuntimePaths::new(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("runtime"))
    }
    #[test]
    fn interrupted_invalid_and_concurrent_updates_preserve_last_good_pair() {
        let data = tempfile::tempdir().unwrap();
        let rt = runtime();
        ensure_initialized(&rt, data.path()).unwrap();
        let previous = std::fs::read(metadata_path(data.path())).unwrap();
        let (meta, ip, site) = snapshot(data.path()).unwrap();
        // Crash after one staged file: published metadata and both active files stay intact.
        let interrupted = tempfile::tempdir_in(geo_dir(data.path())).unwrap();
        std::fs::write(interrupted.path().join("geoip.dat"), b"partial").unwrap();
        ensure_initialized(&rt, data.path()).unwrap();
        assert_eq!(std::fs::read(metadata_path(data.path())).unwrap(), previous);
        assert!(info(data.path()).valid);
        assert!(install_snapshot(data.path(), &rt.xray_binary(), meta.clone(), &ip, b"invalid").is_err());
        assert!(install_snapshot(data.path(), &rt.xray_dir().join("absent.exe"), meta.clone(), &ip, &site).is_err());
        let guard = lock(data.path()).unwrap();
        assert!(install_snapshot(data.path(), &rt.xray_binary(), meta.clone(), &ip, &site).is_err());
        drop(guard);
        assert_eq!(std::fs::read(metadata_path(data.path())).unwrap(), previous);
        install_snapshot(data.path(), &rt.xray_binary(), meta, &ip, &site).unwrap();
        assert_ne!(std::fs::read(metadata_path(data.path())).unwrap(), previous);
        restore_metadata(data.path(), &previous).unwrap();
        assert!(info(data.path()).valid);
    }
    #[test]
    fn bundled_and_updated_assets_are_verified_and_resolved_from_same_generation() {
        let data = tempfile::tempdir().unwrap();
        let rt = runtime();
        let core = crate::xray::core::prepare(&rt, data.path()).unwrap();
        assert_eq!(assets_for_core(&core), active_dir(data.path()).unwrap());
        let (meta, ip, site) = snapshot(data.path()).unwrap();
        install_snapshot(data.path(), &core, meta, &ip, &site).unwrap();
        assert_eq!(assets_for_core(&core), active_dir(data.path()).unwrap());
        std::fs::write(active_dir(data.path()).unwrap().join("geosite.dat"), vec![1; 2000]).unwrap();
        assert!(!info(data.path()).valid);
        let asset = Asset { name:"geoip.dat".into(), size:1024, browser_download_url:String::new(), digest:Some(format!("sha256:{}", "a".repeat(64))) };
        assert!(verify_download(&asset, &"b".repeat(64)).is_err());
    }
}
