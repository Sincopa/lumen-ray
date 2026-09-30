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
