use super::parser::MAX_BYTES;
use crate::domain::errors::{BackendError, Result};
use base64::Engine;
use std::time::Duration;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SubscriptionUserInfo {
    pub upload: Option<u64>,
    pub download: Option<u64>,
    pub total: Option<u64>,
    pub expire: Option<u64>,
}

pub struct FetchedSubscription {
    pub body: String,
    pub user_info: Option<SubscriptionUserInfo>,
    pub suggested_name: Option<String>,
}

fn clean_title(value: &str) -> Option<String> {
    let decoded = percent_encoding::percent_decode_str(value.trim().trim_matches(['"', '\'']))
        .decode_utf8_lossy();
    let cleaned: String = decoded
        .chars()
        .filter(|character| !character.is_control())
        .take(80)
        .collect();
    let cleaned = cleaned.trim();
    let lower = cleaned.to_ascii_lowercase();
    let suffix_length = [".yaml", ".yml", ".txt"]
        .into_iter()
        .find(|suffix| lower.ends_with(suffix))
        .map(str::len)
        .unwrap_or(0);
    let cleaned = &cleaned[..cleaned.len() - suffix_length];
    (!cleaned.is_empty()).then(|| cleaned.to_owned())
}

pub fn parse_profile_title(value: &str) -> Option<String> {
    let value = value.trim();
    if let Some(encoded) = value.strip_prefix("base64:") {
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(encoded.trim())
            .or_else(|_| base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(encoded.trim()))
            .ok()?;
        return clean_title(&String::from_utf8(decoded).ok()?);
    }
    clean_title(value)
}

pub fn parse_content_disposition(value: &str) -> Option<String> {
    let mut fallback = None;
    for part in value.split(';').map(str::trim) {
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        if key.eq_ignore_ascii_case("filename*") {
            let encoded = value
                .trim()
                .strip_prefix("UTF-8''")
                .or_else(|| value.trim().strip_prefix("utf-8''"))
                .unwrap_or(value.trim());
            return clean_title(encoded);
        }
        if key.eq_ignore_ascii_case("filename") {
            fallback = clean_title(value);
        }
    }
    fallback
}

pub fn parse_user_info(value: &str) -> Option<SubscriptionUserInfo> {
    let mut info = SubscriptionUserInfo::default();
    let mut found = false;
    for part in value.split(';') {
        let Some((key, value)) = part.trim().split_once('=') else {
            continue;
        };
        let Ok(value) = value.trim().parse::<u64>() else {
            continue;
        };
        match key.trim().to_ascii_lowercase().as_str() {
            "upload" => info.upload = Some(value),
            "download" => info.download = Some(value),
            "total" => info.total = Some(value),
            "expire" => info.expire = Some(value),
            _ => continue,
        }
        found = true;
    }
    found.then_some(info)
}

pub async fn fetch_with_metadata(source: &str) -> Result<FetchedSubscription> {
    let source = source
        .trim()
        .trim_start_matches('\u{feff}')
        .trim_matches(['"', '\'', '<', '>'])
        .trim();
    if source.len() > MAX_BYTES {
        return Err(BackendError::invalid());
    }
    let normalized_source: String;
    let url_candidate = if !source.contains("://")
        && !source.starts_with(['{', '['])
        && !source.contains("proxies:")
        && source.contains('.')
        && source.contains('/')
        && !source.contains(' ')
    {
        normalized_source = format!("https://{source}");
        normalized_source.as_str()
    } else {
        source
    };

    let is_http = url_candidate.starts_with("https://") || url_candidate.starts_with("http://");
    if !is_http {
        return Ok(FetchedSubscription {
            body: source.to_owned(),
            user_info: None,
            suggested_name: None,
        });
    }
    let url = url::Url::parse(url_candidate).map_err(|_| BackendError::invalid())?;
    let error = || {
        BackendError::new(
            "SUBSCRIPTION_FETCH_FAILED",
            "Unable to download subscription",
        )
    };
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::limited(5))
        .user_agent("Lumen/0.1 Clash/1.19 Xray/26.9.9")
        .build()
        .map_err(|_| error())?;
    let mut response = client
        .get(url)
        .send()
        .await
        .map_err(|_| error())?
        .error_for_status()
        .map_err(|_| error())?;
    let user_info = response.headers().iter().find_map(|(name, value)| {
        name.as_str()
            .ends_with("subscription-userinfo")
            .then(|| value.to_str().ok().and_then(parse_user_info))
            .flatten()
    });
    let suggested_name = response
        .headers()
        .get(reqwest::header::CONTENT_DISPOSITION)
        .and_then(|value| value.to_str().ok())
        .and_then(parse_content_disposition)
        .or_else(|| {
            response
                .headers()
                .iter()
                .find(|(name, _)| name.as_str().eq_ignore_ascii_case("profile-title"))
                .and_then(|(_, value)| value.to_str().ok())
                .and_then(parse_profile_title)
        });
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
    Ok(FetchedSubscription {
        body: String::from_utf8(body).map_err(|_| BackendError::invalid())?,
        user_info,
        suggested_name,
    })
}

#[allow(dead_code)]
pub async fn fetch(source: &str) -> Result<String> {
    fetch_with_metadata(source).await.map(|f| f.body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_subscription_user_info_with_or_without_spaces() {
        let expected = SubscriptionUserInfo {
            upload: Some(1_718_022_024),
            download: Some(45_069_371_798),
            total: Some(274_877_906_944),
            expire: Some(1_805_938_734),
        };
        assert_eq!(
            parse_user_info(
                "upload=1718022024; download=45069371798; total=274877906944; expire=1805938734"
            ),
            Some(expected)
        );
        assert_eq!(
            parse_user_info(
                "upload=1718022024;download=45069371798;total=274877906944;expire=1805938734"
            ),
            Some(expected)
        );
    }

    #[test]
    fn parses_mihomo_profile_names() {
        assert_eq!(
            parse_profile_title("base64:RXhhbXBsZSBWUE4="),
            Some("Example VPN".into())
        );
        assert_eq!(
            parse_content_disposition("inline; filename*=UTF-8''Koala%20VPN.yaml"),
            Some("Koala VPN".into())
        );
        assert_eq!(
            parse_content_disposition("attachment; filename=\"Personal.yml\""),
            Some("Personal".into())
        );
    }

    #[tokio::test]
    async fn raw_keys_bypass_network_fetch_and_strip_quotes() {
        let raw = "\"vless://uuid@1.1.1.1:443?type=tcp#Test\"";
        let fetched = fetch_with_metadata(raw).await.unwrap();
        assert_eq!(fetched.body, "vless://uuid@1.1.1.1:443?type=tcp#Test");
    }
}
