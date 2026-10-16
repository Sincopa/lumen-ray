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
    /// Incy `routing` header, when the response included one.
    pub routing: Option<String>,
}

pub fn header_value_to_string(val: &reqwest::header::HeaderValue) -> Option<String> {
    val.to_str()
        .ok()
        .map(str::to_string)
        .or_else(|| String::from_utf8(val.as_bytes().to_vec()).ok())
}

pub fn is_generic_subscription_filename(name: &str) -> bool {
    let trimmed = name.trim();
    [
        "config",
        "sub",
        "subscription",
        "subscriptions",
        "proxies",
        "proxy",
        "clash",
        "mihomo",
        "default",
        "data",
        "profile",
        "export",
        "all",
        "nodes",
        "servers",
        "vpn",
        "v2ray",
        "xray",
        "singbox",
        "sing-box",
    ]
    .iter()
    .any(|&generic| trimmed.eq_ignore_ascii_case(generic))
}

fn clean_title(value: &str) -> Option<String> {
    let raw = value.trim().trim_matches(['"', '\'']).trim();
    let unescaped = if let Some(s) = raw
        .strip_prefix("UTF-8''")
        .or_else(|| raw.strip_prefix("utf-8''"))
    {
        s
    } else {
        raw
    };
    let decoded = percent_encoding::percent_decode_str(unescaped).decode_utf8_lossy();
    let cleaned: String = decoded
        .chars()
        .filter(|character| !character.is_control())
        .take(80)
        .collect();
    let cleaned = cleaned.trim();
    let suffix_length = [".yaml", ".yml", ".txt", ".json", ".conf"]
        .into_iter()
        .find(|suffix| {
            cleaned
                .get(cleaned.len().saturating_sub(suffix.len())..)
                .is_some_and(|tail| tail.eq_ignore_ascii_case(suffix))
        })
        .map(str::len)
        .unwrap_or(0);
    let cleaned = &cleaned[..cleaned.len() - suffix_length];
    let cleaned = cleaned.trim();
    (!cleaned.is_empty()).then(|| cleaned.to_owned())
}

pub fn parse_profile_title(value: &str) -> Option<String> {
    let value = value.trim().trim_matches(['"', '\'']).trim();
    if let Some(encoded) = value
        .strip_prefix("base64:")
        .or_else(|| value.strip_prefix("BASE64:"))
        .or_else(|| value.strip_prefix("base64,"))
    {
        let encoded = encoded.trim();
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(encoded))
            .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(encoded))
            .or_else(|_| base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(encoded))
            .ok()?;
        return clean_title(&String::from_utf8(decoded).ok()?);
    }
    if !value.contains(' ') && value.len() >= 4 && value.len().is_multiple_of(4) {
        if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(value) {
            if let Ok(s) = String::from_utf8(decoded) {
                if s.chars().all(|c| !c.is_control()) && s.chars().any(|c| c.is_alphabetic()) {
                    if let Some(title) = clean_title(&s) {
                        return Some(title);
                    }
                }
            }
        }
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

pub fn extract_title_from_body(body: &str) -> Option<String> {
    for line in body.lines().take(25) {
        let trimmed = line.trim();
        let stripped = trimmed
            .strip_prefix('#')
            .or_else(|| trimmed.strip_prefix("//"))
            .map(str::trim);
        if let Some(content) = stripped {
            if let Some((k, v)) = content.split_once(':') {
                let k_trimmed = k.trim();
                if [
                    "profile-title",
                    "subscription-title",
                    "profile-name",
                    "profile",
                    "name",
                ]
                .iter()
                .any(|&candidate| k_trimmed.eq_ignore_ascii_case(candidate))
                {
                    if let Some(title) = parse_profile_title(v.trim()) {
                        return Some(title);
                    }
                }
            }
        }
    }
    None
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
        let k = key.trim();
        if k.eq_ignore_ascii_case("upload") {
            info.upload = Some(value);
        } else if k.eq_ignore_ascii_case("download") {
            info.download = Some(value);
        } else if k.eq_ignore_ascii_case("total") {
            info.total = Some(value);
        } else if k.eq_ignore_ascii_case("expire") {
            info.expire = Some(value);
        } else {
            continue;
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

    pub fn through_proxy(&self, port: u16) -> Result<Self> {
        let client = reqwest::Client::builder()
            .no_proxy()
            .proxy(
                reqwest::Proxy::all(format!("socks5h://127.0.0.1:{port}"))
                    .map_err(|_| BackendError::invalid())?,
            )
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| {
                BackendError::new(
                    "SUBSCRIPTION_FETCH_FAILED",
                    "Could not initialize subscription transport",
                )
            })?;
        Ok(Self {
            client,
            identity_provider: self.identity_provider.clone(),
            user_agent: self.user_agent.clone(),
        })
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

        // Pasted keys are not fetched, and they do not get device headers.
        if !is_http {
            return Ok(FetchedSubscription {
                body: source.to_owned(),
                user_info: None,
                suggested_name: None,
                routing: None,
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

            // Device headers stay on the subscription host. Another origin does not get them.
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

                // Do not follow an HTTPS URL onto plain HTTP.
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

        // Device-limit headers are checked before the HTTP status.
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

        // Other client errors, except 429.
        if response.status().is_client_error() && response.status().as_u16() != 429 {
            return Err(BackendError::new(
                "SUBSCRIPTION_ACCESS_DENIED",
                "The subscription service rejected the request",
            ));
        }
        response = response.error_for_status().map_err(|_| error())?;

        let user_info = response.headers().iter().find_map(|(name, value)| {
            name.as_str()
                .ends_with("subscription-userinfo")
                .then(|| value.to_str().ok().and_then(parse_user_info))
                .flatten()
        });
        let routing = response
            .headers()
            .iter()
            .find(|(name, _)| name.as_str().eq_ignore_ascii_case("routing"))
            .and_then(|(_, value)| header_value_to_string(value))
            .filter(|value| !value.trim().is_empty());

        let title_headers = [
            "profile-title",
            "subscription-title",
            "x-profile-title",
            "profile-name",
            "web-page-title",
        ];

        let mut suggested_name = response
            .headers()
            .iter()
            .find(|(name, _)| {
                title_headers
                    .iter()
                    .any(|h| name.as_str().eq_ignore_ascii_case(h))
            })
            .and_then(|(_, value)| header_value_to_string(value))
            .and_then(|v| parse_profile_title(&v))
            .or_else(|| {
                response
                    .headers()
                    .get(reqwest::header::CONTENT_DISPOSITION)
                    .and_then(header_value_to_string)
                    .and_then(|v| parse_content_disposition(&v))
                    .filter(|name| !is_generic_subscription_filename(name))
            });

        if response
            .content_length()
            .is_some_and(|n| n > MAX_BYTES as u64)
        {
            return Err(BackendError::invalid());
        }

        let capacity = response
            .content_length()
            .map(|n| (n as usize).min(MAX_BYTES))
            .unwrap_or(0);
        let mut body = Vec::with_capacity(capacity);
        while let Some(chunk) = response.chunk().await.map_err(|_| error())? {
            if body.len() + chunk.len() > MAX_BYTES {
                return Err(BackendError::invalid());
            }
            body.extend_from_slice(&chunk);
        }

        let body_str = String::from_utf8(body).map_err(|_| BackendError::invalid())?;
        if suggested_name.is_none() {
            suggested_name = extract_title_from_body(&body_str);
        }

        Ok(FetchedSubscription {
            body: body_str,
            user_info,
            suggested_name,
            routing,
        })
    }

    pub async fn fetch(&self, source: &str) -> Result<String> {
        self.fetch_with_metadata(source).await.map(|f| f.body)
    }
}

/// Helper function to check if a header exists case-insensitively and has value "true" or "1".
fn has_header_flag(headers: &reqwest::header::HeaderMap, name: &str) -> bool {
    headers.get_all(name).iter().any(|val| {
        val.to_str()
            .map(|s| {
                let trimmed = s.trim();
                trimmed.eq_ignore_ascii_case("true") || trimmed == "1"
            })
            .unwrap_or(false)
    })
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
        // Base64 with cyrillic characters
        assert_eq!(
            parse_profile_title("base64:0JPRgNC40LHQvdC40Lo="),
            Some("Грибник".into())
        );
        assert_eq!(
            parse_profile_title("BASE64:0JPRgNC40LHQvdC40Lo="),
            Some("Грибник".into())
        );
        assert_eq!(
            parse_profile_title("\"base64:0JPRgNC40LHQvdC40Lo=\""),
            Some("Грибник".into())
        );
        // Raw base64 without prefix
        assert_eq!(
            parse_profile_title("0JPRgNC40LHQvdC40Lo="),
            Some("Грибник".into())
        );
        // Percent-encoded
        assert_eq!(
            parse_profile_title("%D0%93%D1%80%D0%B8%D0%B1%D0%BD%D0%B8%D0%BA"),
            Some("Грибник".into())
        );
        assert_eq!(
            parse_content_disposition("inline; filename*=UTF-8''Koala%20VPN.yaml"),
            Some("Koala VPN".into())
        );
        assert_eq!(
            parse_content_disposition("attachment; filename=\"Personal.yml\""),
            Some("Personal".into())
        );
        assert!(is_generic_subscription_filename("config"));
        assert!(is_generic_subscription_filename("sub"));
        assert!(is_generic_subscription_filename("clash"));
        assert!(is_generic_subscription_filename("proxies"));
        assert!(!is_generic_subscription_filename("MyCustomService"));

        let body_with_title = "# profile-title: base64:0JPRgNC40LHQvdC40Lo=\nvless://...";
        assert_eq!(
            extract_title_from_body(body_with_title),
            Some("Грибник".into())
        );

        let body_with_plain = "// subscription-title: CoolVPN\nvless://...";
        assert_eq!(
            extract_title_from_body(body_with_plain),
            Some("CoolVPN".into())
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
