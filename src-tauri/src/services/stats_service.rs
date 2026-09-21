use crate::domain::errors::{BackendError, Result};
use serde::Serialize;
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrafficStats {
    pub available: bool,
    pub upload: u64,
    pub download: u64,
    pub upload_speed: f64,
    pub download_speed: f64,
    pub duration_seconds: u64,
}
pub async fn counters(client: &reqwest::Client, port: u16) -> Result<(u64, u64)> {
    let error = || {
        BackendError::new(
            "METRICS_UNAVAILABLE",
            "Xray traffic metrics are unavailable",
        )
    };
    let response = client
        .get(format!("http://127.0.0.1:{port}/debug/vars"))
        .send()
        .await
        .map_err(|_| error())?;
    let data: serde_json::Value = response.json().await.map_err(|_| error())?;
    let outbound = data["stats"]["outbound"].as_object().ok_or_else(error)?;
    let mut up = 0u64;
    let mut down = 0u64;
    for (tag, stats) in outbound {
        if !["direct", "block", "metrics"].contains(&tag.as_str()) {
            up = up.saturating_add(stats["uplink"].as_u64().unwrap_or(0));
            down = down.saturating_add(stats["downlink"].as_u64().unwrap_or(0));
        }
    }
    Ok((up, down))
}
