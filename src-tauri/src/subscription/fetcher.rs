use super::parser::MAX_BYTES;
use crate::{
    domain::errors::{BackendError, Result},
    platform::device_identity::{is_safe_header_value, DeviceIdentity, DeviceIdentityManager},
};
use base64::Engine;
use std::{sync::Arc, time::Duration};

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

/// Builds the standardized User-Agent containing the real Lumen Cargo version
/// and structured compatibility tokens.
pub fn build_user_agent() -> String {
    format!(
        "Lumen/{} Clash/1.19 Xray/{}",
        env!("CARGO_PKG_VERSION"),
        crate::xray::core::VERSION
    )
}

/// Trait abstracting device identity resolution for subscription HTTP requests.
pub trait DeviceIdentityProvider: Send + Sync {
    fn get_identity(&self) -> Result<DeviceIdentity>;
}

impl DeviceIdentityProvider for DeviceIdentityManager {
    fn get_identity(&self) -> Result<DeviceIdentity> {
        self.get_or_init()
    }
}

impl DeviceIdentityProvider for DeviceIdentity {
    fn get_identity(&self) -> Result<DeviceIdentity> {
        Ok(self.clone())
    }
}

/// Dedicated HTTP client for subscription operations with HWID and redirect privacy support.
#[derive(Clone)]
pub struct SubscriptionClient {
    client: reqwest::Client,
    identity_provider: Arc<dyn DeviceIdentityProvider>,
    user_agent: String,
}

impl SubscriptionClient {
    pub fn new(identity_provider: Arc<dyn DeviceIdentityProvider>) -> Self {
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap_or_default();

        Self {
            client,
            identity_provider,
            user_agent: build_user_agent(),
        }
    }

    pub fn with_identity(identity: DeviceIdentity) -> Self {
        Self::new(Arc::new(identity))
    }

    /// Creates an ephemeral subscription client for testing or standalone smoke verification.
    pub fn ephemeral() -> Result<Self> {
        let seed = uuid::Uuid::new_v4().to_string();
        let hwid = crate::platform::device_identity::generate_hwid_from_seed(&seed);
        let identity = DeviceIdentity {
            hwid,
            device_os: "Windows".to_string(),
            ver_os: crate::platform::device_identity::read_os_version(),
            device_model: crate::platform::device_identity::read_device_model(),
        };
        Ok(Self::with_identity(identity))
    }

    pub fn user_agent(&self) -> &str {
        &self.user_agent
    }

    /// Fetches a subscription and its metadata, applying HWID headers and redirect privacy.
    pub async fn fetch_with_metadata(&self, source: &str) -> Result<FetchedSubscription> {
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

        // Never send HTTP requests or HWID headers to inline/pasted subscriptions
        if !is_http {
            return Ok(FetchedSubscription {
                body: source.to_owned(),
                user_info: None,
                suggested_name: None,
            });
        }

        let initial_url = url::Url::parse(url_candidate).map_err(|_| BackendError::invalid())?;
        let identity = self.identity_provider.get_identity()?;

        // Header injection and safety validation
        if !is_safe_header_value(&identity.hwid)
            || !is_safe_header_value(&identity.device_os)
            || !is_safe_header_value(&identity.ver_os)
            || !is_safe_header_value(&self.user_agent)
        {
            return Err(BackendError::invalid());
        }

        if let Some(ref model) = identity.device_model {
            if !is_safe_header_value(model) {
                return Err(BackendError::invalid());
            }
        }

        let error = || {
            BackendError::new(
                "SUBSCRIPTION_FETCH_FAILED",
                "Unable to download subscription",
            )
        };

        let initial_origin = initial_url.origin();
        let mut current_url = initial_url;
        let mut redirect_count = 0;
        let mut response: reqwest::Response;

        loop {
            let is_same_origin = current_url.origin() == initial_origin;
            let mut request_builder = self.client.get(current_url.clone());

            let ua_val = reqwest::header::HeaderValue::from_str(&self.user_agent)
                .map_err(|_| BackendError::invalid())?;
            request_builder = request_builder.header(reqwest::header::USER_AGENT, ua_val);

            // Forward HWID and device headers ONLY to the original target origin
            if is_same_origin {
                let hwid_val = reqwest::header::HeaderValue::from_str(&identity.hwid)
                    .map_err(|_| BackendError::invalid())?;
                let os_val = reqwest::header::HeaderValue::from_str(&identity.device_os)
                    .map_err(|_| BackendError::invalid())?;
                let ver_val = reqwest::header::HeaderValue::from_str(&identity.ver_os)
                    .map_err(|_| BackendError::invalid())?;

                request_builder = request_builder
                    .header("x-hwid", hwid_val)
                    .header("x-device-os", os_val)
                    .header("x-ver-os", ver_val);

                if let Some(ref model) = identity.device_model {
                    if let Ok(model_val) = reqwest::header::HeaderValue::from_str(model) {
                        request_builder = request_builder.header("x-device-model", model_val);
                    }
                }
            }

            let resp = request_builder.send().await.map_err(|_| error())?;
            let status = resp.status();

            if status.is_redirection() {
                if redirect_count >= 5 {
                    return Err(BackendError::new(
                        "SUBSCRIPTION_FETCH_FAILED",
                        "Too many redirects encountered while downloading subscription",
                    ));
                }

                let location = resp
                    .headers()
                    .get(reqwest::header::LOCATION)
                    .and_then(|v| v.to_str().ok())
                    .ok_or_else(error)?;

                let next_url = current_url.join(location).map_err(|_| error())?;

                // Enforce HTTPS security: disallow downgrade to HTTP
                if current_url.scheme() == "https" && next_url.scheme() != "https" {
                    return Err(BackendError::new(
                        "SUBSCRIPTION_FETCH_FAILED",
                        "Insecure redirect from HTTPS to HTTP is forbidden",
                    ));
                }

                current_url = next_url;
                redirect_count += 1;
                continue;
            }

            response = resp;
            break;
        }

        // Case-insensitively inspect response headers for Remnawave error flags before checking HTTP status or body
        let headers = response.headers();
        if has_header_flag(headers, "x-hwid-not-supported") {
            return Err(BackendError::new(
                "HWID_NOT_SUPPORTED",
                "Subscription service did not accept the device identifier. Please update the application or contact your subscription provider.",
            ));
        }

        if has_header_flag(headers, "x-hwid-max-devices-reached")
            || has_header_flag(headers, "x-hwid-limit")
        {
            return Err(BackendError::new(
                "HWID_DEVICE_LIMIT",
                "Device limit reached for this subscription. Remove an old device in your dashboard or contact your provider.",
            ));
        }

        // If no Remnawave HWID error headers were received, verify standard HTTP status
        response = response.error_for_status().map_err(|_| error())?;

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

    pub async fn fetch(&self, source: &str) -> Result<String> {
        self.fetch_with_metadata(source).await.map(|f| f.body)
    }
}

/// Helper function to check if a header exists case-insensitively and has value "true" or "1".
fn has_header_flag(headers: &reqwest::header::HeaderMap, name: &str) -> bool {
    for (key, val) in headers {
        if key.as_str().eq_ignore_ascii_case(name) {
            if let Ok(str_val) = val.to_str() {
                let trimmed = str_val.trim();
                if trimmed.eq_ignore_ascii_case("true") || trimmed == "1" {
                    return true;
                }
            }
        }
    }
    false
}

#[allow(dead_code)]
pub async fn fetch_with_metadata(
    source: &str,
    client: &SubscriptionClient,
) -> Result<FetchedSubscription> {
    client.fetch_with_metadata(source).await
}

#[allow(dead_code)]
pub async fn fetch(source: &str, client: &SubscriptionClient) -> Result<String> {
    client.fetch(source).await
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
        let client = SubscriptionClient::ephemeral().unwrap();
        let raw = "\"vless://uuid@1.1.1.1:443?type=tcp#Test\"";
        let fetched = client.fetch_with_metadata(raw).await.unwrap();
        assert_eq!(fetched.body, "vless://uuid@1.1.1.1:443?type=tcp#Test");
    }

    #[test]
    fn user_agent_contains_actual_versions() {
        let ua = build_user_agent();
        assert!(ua.starts_with(&format!("Lumen/{}", env!("CARGO_PKG_VERSION"))));
        assert!(ua.contains(&format!("Xray/{}", crate::xray::core::VERSION)));
        assert!(ua.contains("Clash/1.19"));
    }
}
