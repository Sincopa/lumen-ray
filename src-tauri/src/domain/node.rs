use super::errors::{BackendError, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    Vless,
    Vmess,
    Trojan,
    Shadowsocks,
    Hysteria,
    Wireguard,
}

/// Credentials never cross IPC. Only the encrypted repository persists this model.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VpnNode {
    pub id: String,
    pub subscription_id: String,
    pub name: String,
    pub address: String,
    pub port: u16,
    pub protocol: Protocol,
    pub credential: String,
    pub method: String,
    pub flow: String,
    pub transport: Transport,
    pub security: Security,
    #[serde(default)]
    pub wireguard: Option<WireguardOptions>,
    #[serde(default)]
    pub balancer: Option<BalancerOptions>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BalancerOptions {
    pub tag: String,
    pub selector: Vec<String>,
    pub strategy: serde_json::Value,
    #[serde(default)]
    pub observatory: Option<serde_json::Value>,
    pub outbounds: Vec<serde_json::Value>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WireguardOptions {
    pub secret_key: String,
    pub address: Vec<String>,
    pub peers: Vec<WireguardPeer>,
    #[serde(default = "default_mtu")]
    pub mtu: u16,
    #[serde(default)]
    pub reserved: Vec<u8>,
    #[serde(default = "default_strategy")]
    pub domain_strategy: String,
    #[serde(default)]
    pub no_kernel_tun: bool,
}
fn default_mtu() -> u16 {
    1420
}
fn default_strategy() -> String {
    "ForceIP".into()
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WireguardPeer {
    pub endpoint: String,
    pub public_key: String,
    #[serde(default)]
    pub pre_shared_key: String,
    #[serde(default)]
    pub keep_alive: u16,
    #[serde(default, rename = "allowedIPs")]
    pub allowed_i_ps: Vec<String>,
}
#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Transport {
    pub network: String,
    pub path: String,
    pub host: String,
    pub service_name: String,
    pub mode: String,
    #[serde(default)]
    pub seed: String,
    #[serde(default)]
    pub header_type: String,
}
#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Security {
    #[serde(default)]
    pub spider_x: String,
    pub kind: String,
    pub server_name: String,
    pub fingerprint: String,
    pub public_key: String,
    pub short_id: String,
    pub alpn: Vec<String>,
}
impl VpnNode {
    pub fn validate(&self) -> Result<()> {
        if self.address.is_empty()
            || self.address.len() > 253
            || self.address.chars().any(char::is_whitespace)
            || self.port == 0
            || self.credential.is_empty()
            || self.credential.len() > 4096
        {
            return Err(BackendError::invalid());
        }
        if ![
            "raw",
            "tcp",
            "ws",
            "grpc",
            "httpupgrade",
            "xhttp",
            "kcp",
            "mkcp",
            "hysteria",
        ]
        .contains(&self.transport.network.as_str())
        {
            return Err(BackendError::unsupported());
        }
        if self.protocol == Protocol::Hysteria
            && (self.transport.network != "hysteria" || self.security.kind != "tls")
        {
            return Err(BackendError::unsupported());
        }
        if !["none", "tls", "reality"].contains(&self.security.kind.as_str()) {
            return Err(BackendError::unsupported());
        }
        if self.security.kind == "reality"
            && (self.security.public_key.is_empty() || self.security.server_name.is_empty())
        {
            return Err(BackendError::invalid());
        }
        if !self.flow.is_empty()
            && (self.protocol != Protocol::Vless || self.flow != "xtls-rprx-vision")
        {
            return Err(BackendError::unsupported());
        }
        Ok(())
    }
    pub fn identify(&mut self) -> Result<()> {
        self.validate()?;
        self.address = self.address.to_lowercase();
        let mut identity = self.clone();
        identity.name.clear();
        identity.id.clear();
        let bytes = serde_json::to_vec(&identity).map_err(|_| BackendError::invalid())?;
        self.id = format!("{:x}", Sha256::digest(bytes));
        Ok(())
    }
}
