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
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeoDatabaseInfo {
    pub version: String,
    pub source: String,
    pub updated_at: u64,
    pub status: String,
    pub error: Option<String>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    size: usize,
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

pub fn info(data: &Path) -> GeoDatabaseInfo {
    let dir = geo_dir(data);
    let meta_file = metadata_path(data);
    let geoip_file = dir.join("geoip.dat");
    let geosite_file = dir.join("geosite.dat");

    if !geoip_file.is_file() || !geosite_file.is_file() {
        return GeoDatabaseInfo {
            version: "none".into(),
            source: GEO_SOURCE_NAME.into(),
            updated_at: 0,
            status: "missing".into(),
            error: Some("Geo databases not found".into()),
        };
    }

    let meta: Option<GeoDatabaseMetadata> = std::fs::read(&meta_file)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok());

    let (version, source, updated_at) = if let Some(m) = meta {
        (m.version, m.source, m.updated_at)
    } else {
        (BUNDLED_GEO_VERSION.into(), GEO_SOURCE_NAME.into(), 0)
    };

    let geoip_size = std::fs::metadata(&geoip_file).map(|m| m.len()).unwrap_or(0);
    let geosite_size = std::fs::metadata(&geosite_file)
        .map(|m| m.len())
        .unwrap_or(0);

    if geoip_size < 1024 || geosite_size < 1024 {
        return GeoDatabaseInfo {
            version,
            source,
            updated_at,
            status: "corrupted".into(),
            error: Some("Database files are corrupted or zero size".into()),
        };
    }

    GeoDatabaseInfo {
        version,
        source,
        updated_at,
        status: "ready".into(),
        error: None,
    }
}

pub fn ensure_initialized(resources: &Path, data: &Path) -> Result<()> {
    let dir = geo_dir(data);
    std::fs::create_dir_all(&dir).map_err(|_| error())?;

    let geoip_dst = dir.join("geoip.dat");
    let geosite_dst = dir.join("geosite.dat");
    let meta_dst = metadata_path(data);

    let needs_copy = !geoip_dst.is_file() || !geosite_dst.is_file() || !meta_dst.is_file();
    if needs_copy {
        let geoip_src = resources.join("geoip.dat");
        let geosite_src = resources.join("geosite.dat");
        if !geoip_src.is_file() || !geosite_src.is_file() {
            return Err(BackendError::new(
                "CORE_NOT_FOUND",
                "Bundled GeoIP/GeoSite database files are missing",
            ));
        }

        let geoip_bytes = std::fs::read(&geoip_src).map_err(|_| error())?;
        let geosite_bytes = std::fs::read(&geosite_src).map_err(|_| error())?;

        let geoip_hash = format!("{:x}", Sha256::digest(&geoip_bytes));
        let geosite_hash = format!("{:x}", Sha256::digest(&geosite_bytes));

        crate::xray::updater::atomic_write(&geoip_dst, &geoip_bytes)?;
        crate::xray::updater::atomic_write(&geosite_dst, &geosite_bytes)?;

        let meta = GeoDatabaseMetadata {
            version: BUNDLED_GEO_VERSION.into(),
            source: GEO_SOURCE_NAME.into(),
            updated_at: now_ms(),
            geoip_sha256: geoip_hash,
            geosite_sha256: geosite_hash,
        };
        let meta_bytes = serde_json::to_vec(&meta).map_err(|_| error())?;
        crate::xray::updater::atomic_write(&meta_dst, &meta_bytes)?;
    }

    Ok(())
}

pub async fn update(data: &Path, core_path: &Path) -> Result<GeoDatabaseInfo> {
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

    // Validate using Xray validator test config with staging directory
    if core_path.is_file() {
        validate_geo_files(core_path, staging.path()).await?;
    }

    // Atomically persist to geo directory
    let geoip_dest = dir.join("geoip.dat");
    let geosite_dest = dir.join("geosite.dat");
    crate::xray::updater::atomic_write(&geoip_dest, &geoip_bytes)?;
    crate::xray::updater::atomic_write(&geosite_dest, &geosite_bytes)?;

    let meta = GeoDatabaseMetadata {
        version: release.tag_name.clone(),
        source: GEO_SOURCE_NAME.into(),
        updated_at: now_ms(),
        geoip_sha256: geoip_hash,
        geosite_sha256: geosite_hash,
    };
    let meta_bytes = serde_json::to_vec(&meta).map_err(|_| error())?;
    crate::xray::updater::atomic_write(&metadata_path(data), &meta_bytes)?;

    // Also update files in the active core directory so Xray finds them immediately
    if let Some(core_dir) = core_path.parent() {
        if core_dir.is_dir() {
            let _ = crate::xray::updater::atomic_write(&core_dir.join("geoip.dat"), &geoip_bytes);
            let _ =
                crate::xray::updater::atomic_write(&core_dir.join("geosite.dat"), &geosite_bytes);
        }
    }

    Ok(GeoDatabaseInfo {
        version: release.tag_name,
        source: GEO_SOURCE_NAME.into(),
        updated_at: meta.updated_at,
        status: "ready".into(),
        error: None,
    })
}

async fn download_file(url: &str, max_size: usize) -> Result<Vec<u8>> {
    let mut response = client()?
        .get(url)
        .send()
        .await
        .map_err(|_| error())?
        .error_for_status()
        .map_err(|_| error())?;

    let mut bytes = Vec::new();
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

async fn validate_geo_files(core_path: &Path, asset_dir: &Path) -> Result<()> {
    let test_config = serde_json::json!({
        "log": { "loglevel": "none" },
        "inbounds": [{ "port": 10808, "protocol": "socks" }],
        "outbounds": [{ "tag": "direct", "protocol": "freedom" }],
        "routing": {
            "domainStrategy": "IPIfNonMatch",
            "rules": [
                { "type": "field", "ip": ["geoip:private"], "outboundTag": "direct" },
                { "type": "field", "domain": ["geosite:antifilter"], "outboundTag": "direct" },
                { "type": "field", "domain": ["geosite:antifilter-community"], "outboundTag": "direct" },
                { "type": "field", "ip": ["geoip:antifilter"], "outboundTag": "direct" },
                { "type": "field", "ip": ["geoip:antifilter-community"], "outboundTag": "direct" },
                { "type": "field", "domain": ["geosite:category-ru"], "outboundTag": "direct" },
                { "type": "field", "ip": ["geoip:ru"], "outboundTag": "direct" }
            ]
        }
    });

    let config_bytes = serde_json::to_vec(&test_config).map_err(|_| error())?;
    let mut cmd = crate::services::vpn_service::command(core_path);
    cmd.args(["run", "-test", "-config", "stdin:"])
        .env("XRAY_LOCATION_ASSET", asset_dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let mut child = cmd.spawn().map_err(|_| error())?;
    if let Some(mut stdin) = child.stdin.take() {
        use tokio::io::AsyncWriteExt;
        let _ = stdin.write_all(&config_bytes).await;
    }

    let output = tokio::time::timeout(Duration::from_secs(10), child.wait_with_output())
        .await
        .map_err(|_| error())?
        .map_err(|_| error())?;

    if !output.status.success() {
        let err_output = format!(
            "{} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        log::error!("Geo database validation failed: {err_output}");
        return Err(BackendError::new(
            "GEO_DATA_CORRUPTED",
            "Candidate GeoIP/GeoSite databases failed Xray validation",
        ));
    }

    Ok(())
}
