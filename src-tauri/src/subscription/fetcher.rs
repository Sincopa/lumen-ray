use super::parser::MAX_BYTES;
use crate::domain::errors::{BackendError, Result};
use std::time::Duration;

pub async fn fetch(source: &str) -> Result<String> {
    if source.len() > MAX_BYTES {
        return Err(BackendError::invalid());
    }
    if !source.starts_with("https://") {
        return Ok(source.to_owned());
    }
    let url = url::Url::parse(source).map_err(|_| BackendError::invalid())?;
    if !url.username().is_empty() || url.password().is_some() {
        return Err(BackendError::invalid());
    }
    let error = || {
        BackendError::new(
            "SUBSCRIPTION_FETCH_FAILED",
            "Unable to download subscription",
        )
    };
    let client = reqwest::Client::builder()
        .https_only(true)
        .no_proxy()
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::limited(5))
        .user_agent("Lumen/0.1 Xray/26.9.9")
        .build()
        .map_err(|_| error())?;
    let mut response = client
        .get(url)
        .send()
        .await
        .map_err(|_| error())?
        .error_for_status()
        .map_err(|_| error())?;
    if response
        .content_length()
        .is_some_and(|n| n > MAX_BYTES as u64)
    {
        return Err(BackendError::invalid());
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| error())? {
        if body.len() + chunk.len() > MAX_BYTES {
            return Err(BackendError::invalid());
        }
        body.extend_from_slice(&chunk);
    }
    String::from_utf8(body).map_err(|_| BackendError::invalid())
}
