use super::secure::{OsSecretStore, SecretStore};
use crate::domain::{
    errors::{BackendError, Result},
    node::VpnNode,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Subscription {
    pub id: String,
    pub name: String,
    pub updated_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upload: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub download: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expire: Option<u64>,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RuleMatcher {
    #[default]
    Domain,
    Application,
    Geosite,
    Geoip,
    /// Catch-all, equivalent to Clash `MATCH`.
    Final,
}

pub fn is_valid_geo_tag(tag: &str) -> bool {
    let s = tag.trim();
    if s.is_empty() || s.len() > 128 {
        return false;
    }
    s.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '!' || c == '@')
}

pub fn normalize_geo_tag(tag: &str, expected_kind: &str) -> Result<String> {
    let trimmed = tag.trim();
    if expected_kind == "geosite"
        && trimmed
            .get(..6)
            .is_some_and(|p| p.eq_ignore_ascii_case("geoip:"))
    {
        return Err(BackendError::new(
            "INVALID_RULE",
            "Mismatched geoip prefix in geosite rule",
        ));
    }
    if expected_kind == "geoip"
        && trimmed
            .get(..8)
            .is_some_and(|p| p.eq_ignore_ascii_case("geosite:"))
    {
        return Err(BackendError::new(
            "INVALID_RULE",
            "Mismatched geosite prefix in geoip rule",
        ));
    }
    let normalized = if trimmed
        .get(..expected_kind.len())
        .is_some_and(|p| p.eq_ignore_ascii_case(expected_kind))
        && trimmed.as_bytes().get(expected_kind.len()) == Some(&b':')
    {
        trimmed[expected_kind.len() + 1..].trim()
    } else {
        trimmed
    };
    if !is_valid_geo_tag(normalized) {
        return Err(BackendError::new(
            "INVALID_RULE",
            "Category name must contain only letters, numbers, hyphens, underscores, exclamation marks, or @",
        ));
    }
    Ok(normalized.to_string())
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub id: String,
    pub name: String,
    pub matcher: RuleMatcher,
    pub server_id: String,
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preset_id: Option<String>,
    pub is_domain: bool,
}

impl<'de> Deserialize<'de> for Rule {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct RuleHelper {
            id: String,
            name: String,
            #[serde(default)]
            matcher: Option<RuleMatcher>,
            #[serde(default)]
            is_domain: Option<bool>,
            server_id: String,
            enabled: bool,
            #[serde(default)]
            preset_id: Option<String>,
        }

        let helper = RuleHelper::deserialize(deserializer)?;
        let matcher = match helper.matcher {
            Some(m) => m,
            None => match helper.is_domain {
                Some(true) => RuleMatcher::Domain,
                Some(false) => RuleMatcher::Application,
                None => RuleMatcher::Domain,
            },
        };
        let is_domain = matcher == RuleMatcher::Domain;
        Ok(Rule {
            id: helper.id,
            name: helper.name,
            matcher,
            server_id: helper.server_id,
            enabled: helper.enabled,
            preset_id: helper.preset_id,
            is_domain,
        })
    }
}
fn default_true() -> bool {
    true
}
fn default_dns() -> String {
    "1.1.1.1".to_string()
}
fn default_update_interval() -> u32 {
    1
}

fn default_ping_url() -> String {
    "http://www.google.com/generate_204".to_string()
}
fn default_ping_policy() -> String {
    "startup".to_string()
}
fn default_server_selection() -> String {
    "manual".to_string()
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    #[serde(default)]
    pub dns_policy: crate::engine::dns::DnsPolicy,
    #[serde(default = "default_subscription_transport")]
    pub subscription_transport: String,
    pub network_mode: String,
    pub auto_connect: bool,
    #[serde(default = "default_true")]
    pub close_to_tray: bool,
    #[serde(default)]
    pub start_minimized: bool,
    #[serde(default)]
    pub auto_start: bool,
    #[serde(default = "default_dns")]
    pub dns_server: String,
    #[serde(default = "default_true")]
    pub bypass_lan: bool,
    #[serde(default = "default_update_interval")]
    pub update_interval_hours: u32,
    #[serde(default = "default_ping_url")]
    pub ping_url: String,
    #[serde(default = "default_true")]
    pub auto_reconnect: bool,
    #[serde(default = "default_server_selection")]
    pub server_selection: String,
    #[serde(default = "default_ping_policy")]
    pub ping_policy: String,
    #[serde(default)]
    pub notify_disconnect: bool,
    #[serde(default)]
    pub notify_reconnect: bool,
    #[serde(default)]
    pub notify_expiry: bool,
    #[serde(default)]
    pub notify_update: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            network_mode: "tun".into(),
            dns_policy: Default::default(),
            subscription_transport: default_subscription_transport(),
            auto_connect: false,
            close_to_tray: true,
            start_minimized: false,
            auto_start: false,
            dns_server: default_dns(),
            bypass_lan: true,
            update_interval_hours: 1,
            ping_url: default_ping_url(),
            auto_reconnect: true,
            server_selection: default_server_selection(),
            ping_policy: default_ping_policy(),
            notify_disconnect: false,
            notify_reconnect: false,
            notify_expiry: false,
            notify_update: false,
        }
    }
}
fn default_subscription_transport() -> String {
    "direct".into()
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Repository {
    #[serde(default)]
    pub subscription_changes: Vec<SubscriptionChange>,
    pub version: u32,
    pub subscriptions: Vec<Subscription>,
    pub nodes: Vec<VpnNode>,
    pub sources: BTreeMap<String, String>,
    pub rules: Vec<Rule>,
    pub selected_server_id: Option<String>,
    pub selected_subscription_id: Option<String>,
    #[serde(default)]
    pub selected_servers: BTreeMap<String, String>,
    pub routing_mode: String,
    #[serde(default)]
    pub settings: Settings,
    /// Servers whose exit did not answer Discord's voice servers on the last
    /// connect, mapped to the route Discord voice and streams take instead:
    /// another server that answered, or `direct`. The rest of Discord keeps the VPN.
    #[serde(default)]
    pub discord_voice_routes: BTreeMap<String, String>,
}
impl Default for Repository {
    fn default() -> Self {
        Self {
            version: 1,
            subscription_changes: vec![],
            subscriptions: vec![],
            nodes: vec![],
            sources: BTreeMap::new(),
            rules: vec![],
            selected_server_id: None,
            selected_subscription_id: None,
            selected_servers: BTreeMap::new(),
            routing_mode: "global".into(),
            settings: Settings::default(),
            discord_voice_routes: BTreeMap::new(),
        }
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionChange {
    pub subscription_id: String,
    pub at: u64,
    pub added: usize,
    pub updated: usize,
    pub removed: usize,
}
fn error() -> BackendError {
    BackendError::new("STORAGE_FAILED", "Could not read or save application data")
}
pub struct RepositoryStore {
    path: PathBuf,
}
impl RepositoryStore {
    pub fn new(directory: &Path) -> Result<Self> {
        std::fs::create_dir_all(directory).map_err(|_| error())?;
        Ok(Self {
            path: directory.join("repository.dpapi"),
        })
    }
    pub fn load(&self) -> Result<Repository> {
        if !self.path.exists() {
            return Ok(Repository::default());
        }
        match self.load_from(&self.path) {
            Err(error) if error.code != "STORAGE_VERSION" => {
                let mut previous = self
                    .load_from(&self.path.with_extension("previous.dpapi"))
                    .map_err(|_| error)?;
                previous.settings.auto_connect = false;
                log::warn!(
                    "Recovered the previous encrypted repository; automatic connection is disabled"
                );
                Ok(previous)
            }
            result => result,
        }
    }
    fn load_from(&self, path: &Path) -> Result<Repository> {
        let encrypted = std::fs::read(path).map_err(|_| error())?;
        self.decode(&encrypted)
    }
    fn decode(&self, encrypted: &[u8]) -> Result<Repository> {
        let bytes = zeroize::Zeroizing::new(OsSecretStore.unprotect(encrypted)?);
        let mut repo: Repository = serde_json::from_slice(&bytes).map_err(|_| error())?;
        if repo.version != 1 {
            return Err(BackendError::new(
                "STORAGE_VERSION",
                "Application data requires a newer application version",
            ));
        }
        if repo.settings.ping_url == "http://cp.cloudflare.com/generate_204"
            || repo.settings.ping_url.trim().is_empty()
        {
            repo.settings.ping_url = default_ping_url();
        }
        if repo.settings.dns_server.trim().is_empty() {
            repo.settings.dns_server = "system".into();
        }
        Ok(repo)
    }
    pub fn save(&self, repo: &Repository) -> Result<()> {
        let bytes = zeroize::Zeroizing::new(serde_json::to_vec(repo).map_err(|_| error())?);
        let encrypted = OsSecretStore.protect(&bytes)?;
        let mut temporary = tempfile::NamedTempFile::new_in(self.path.parent().ok_or_else(error)?)
            .map_err(|_| error())?;
        temporary.write_all(&encrypted).map_err(|_| error())?;
        temporary.as_file().sync_all().map_err(|_| error())?;
        if let Ok(previous) = std::fs::read(&self.path) {
            if self.decode(&previous).is_ok() {
                crate::xray::updater::atomic_write(
                    &self.path.with_extension("previous.dpapi"),
                    &previous,
                )?;
            }
        }
        temporary.persist(&self.path).map_err(|_| error())?;
        Ok(())
    }
}
