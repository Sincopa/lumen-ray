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

#[derive(Clone, Serialize, Deserialize)]
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
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Rule {
    pub id: String,
    pub name: String,
    pub is_domain: bool,
    pub server_id: String,
    pub enabled: bool,
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

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
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
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            network_mode: "tun".into(),
            auto_connect: false,
            close_to_tray: true,
            start_minimized: false,
            auto_start: false,
            dns_server: default_dns(),
            bypass_lan: true,
            update_interval_hours: 1,
            ping_url: default_ping_url(),
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Repository {
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
}
impl Default for Repository {
    fn default() -> Self {
        Self {
            version: 1,
            subscriptions: vec![],
            nodes: vec![],
            sources: BTreeMap::new(),
            rules: vec![],
            selected_server_id: None,
            selected_subscription_id: None,
            selected_servers: BTreeMap::new(),
            routing_mode: "global".into(),
            settings: Settings::default(),
        }
    }
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
        let encrypted = std::fs::read(&self.path).map_err(|_| error())?;
        let bytes = OsSecretStore.unprotect(&encrypted)?;
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
        Ok(repo)
    }
    pub fn save(&self, repo: &Repository) -> Result<()> {
        let bytes = serde_json::to_vec(repo).map_err(|_| error())?;
        let encrypted = OsSecretStore.protect(&bytes)?;
        let mut temporary = tempfile::NamedTempFile::new_in(self.path.parent().ok_or_else(error)?)
            .map_err(|_| error())?;
        temporary.write_all(&encrypted).map_err(|_| error())?;
        temporary.as_file().sync_all().map_err(|_| error())?;
        temporary.persist(&self.path).map_err(|_| error())?;
        Ok(())
    }
}
