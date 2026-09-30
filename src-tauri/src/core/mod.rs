//! Engine boundary. Backend selection is centralized; an active session owns its engine.
pub mod dns;
pub mod lumen;
pub mod xray;

use crate::{app::state::AppState, domain::errors::Result, storage::repository::Repository};
use serde::{Deserialize, Serialize};
use std::{future::Future, pin::Pin, sync::Arc, time::Instant};

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CoreEngine {
    Lumen,
    #[default]
    Xray,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreCapabilities {
    pub engine: CoreEngine,
    pub experimental: bool,
    pub tun: bool,
    pub system_proxy: bool,
    pub udp: bool,
    pub live_routing_updates: bool,
    pub protocols: Vec<&'static str>,
    pub geosite: bool,
    pub geoip: bool,
    pub application_routing: bool,
    pub reality: bool,
    pub xtls_vision: bool,
    pub kill_switch: bool,
}

pub type BackendFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;

pub trait CoreBackend: Send + Sync {
    fn capabilities(&self) -> CoreCapabilities;
    fn validate(&self, repo: &Repository, selected: &str) -> Result<()>;
    fn validate_runtime<'a>(
        &'a self,
        _state: &'a AppState,
        repo: &'a Repository,
        selected: &'a str,
    ) -> BackendFuture<'a, ()> {
        Box::pin(async move { self.validate(repo, selected) })
    }
    fn connect<'a>(
        &self,
        state: &'a Arc<AppState>,
        repo: &'a Repository,
        selected: &'a str,
        session: &'a mut BackendSession,
    ) -> BackendFuture<'a, ()>;
    fn prewarm<'a>(
        &self,
        state: &'a Arc<AppState>,
        repo: &'a Repository,
        selected: Option<&'a str>,
        session: &'a mut BackendSession,
    ) -> BackendFuture<'a, ()>;
    fn disconnect<'a>(
        &self,
        session: &'a mut BackendSession,
        keep_warm: bool,
    ) -> BackendFuture<'a, ()>;
    fn update_routing<'a>(
        &self,
        state: &'a Arc<AppState>,
        selected: &'a str,
        before: &'a Repository,
        after: &'a Repository,
        session: &'a mut BackendSession,
    ) -> BackendFuture<'a, bool>;
}

pub fn backend(engine: CoreEngine) -> &'static dyn CoreBackend {
    match engine {
        CoreEngine::Lumen => &lumen::LumenBackend,
        CoreEngine::Xray => &xray::XrayBackend,
    }
}

pub enum BackendSession {
    Xray(Box<xray::XraySession>),
    Lumen(Option<lumen::NativeSession>),
}
impl BackendSession {
    pub fn empty(engine: CoreEngine) -> Self {
        match engine {
            CoreEngine::Xray => Self::Xray(Box::default()),
            CoreEngine::Lumen => Self::Lumen(None),
        }
    }
}

pub struct Session {
    pub generation: u64,
    pub backend: BackendSession,
    #[cfg(windows)]
    pub system_proxy: Option<crate::platform::proxy::SystemProxy>,
}
impl Default for Session {
    fn default() -> Self {
        Self {
            generation: 0,
            backend: BackendSession::empty(CoreEngine::Xray),
            #[cfg(windows)]
            system_proxy: None,
        }
    }
}
impl Session {
    pub(crate) fn rollback_repository(&self, repo: &Repository) -> Repository {
        let mut previous = repo.clone();
        previous.settings.core_engine = self.engine();
        previous.settings.network_mode = if self.is_tun() { "tun" } else { "proxy" }.into();
        previous
    }

    pub fn engine(&self) -> CoreEngine {
        match self.backend {
            BackendSession::Xray(_) => CoreEngine::Xray,
            BackendSession::Lumen(_) => CoreEngine::Lumen,
        }
    }
    pub fn started(&self) -> Option<Instant> {
        match &self.backend {
            BackendSession::Xray(s) => s.started,
            BackendSession::Lumen(s) => s.as_ref().map(|s| s.started),
        }
    }
    pub fn proxy_port(&self) -> u16 {
        match &self.backend {
            BackendSession::Xray(s) => s.proxy_port,
            BackendSession::Lumen(s) => s.as_ref().map_or(0, |s| s.port),
        }
    }
    pub fn http_port(&self) -> u16 {
        match &self.backend {
            BackendSession::Xray(s) => s.metrics_port + 1,
            BackendSession::Lumen(s) => s.as_ref().map_or(0, |s| s.port),
        }
    }
    pub fn is_tun(&self) -> bool {
        matches!(&self.backend, BackendSession::Xray(s) if s.is_tun)
    }
    pub fn has_system_proxy(&self) -> bool {
        #[cfg(windows)]
        {
            self.system_proxy.is_some()
        }
        #[cfg(not(windows))]
        {
            false
        }
    }
    /// Process/task liveness is separate from the end-to-end path verification.
    pub fn failed(&mut self) -> bool {
        match &mut self.backend {
            BackendSession::Xray(s) => xray::failed(s),
            BackendSession::Lumen(s) => s.as_ref().is_some_and(|s| s.finished()),
        }
    }
    pub fn stats_source(&self) -> Option<StatsSource> {
        self.started()?;
        match &self.backend {
            BackendSession::Xray(s) => Some(StatsSource::Xray(s.metrics_port)),
            BackendSession::Lumen(s) => s.as_ref().map(|s| StatsSource::Lumen(s.counters.clone())),
        }
    }
}
pub enum StatsSource {
    Xray(u16),
    Lumen(Arc<lumen::Counters>),
}
impl StatsSource {
    pub async fn counters(&self, client: &reqwest::Client) -> Result<(u64, u64)> {
        match self {
            Self::Xray(port) => xray::counters(client, *port).await,
            Self::Lumen(c) => Ok(c.read()),
        }
    }
}

/// The active backend is never inferred from mutable saved settings.
pub async fn connect(
    state: &Arc<AppState>,
    repo: &Repository,
    selected: &str,
    session: &mut Session,
) -> Result<()> {
    let engine = repo.settings.core_engine;
    let selected_backend = backend(engine);
    selected_backend.validate(repo, selected)?;
    if session.engine() != engine {
        crate::services::vpn_service::stop(session).await?;
        session.backend = BackendSession::empty(engine);
    }
    #[cfg(windows)]
    if let Some(proxy) = session.system_proxy.as_ref() {
        proxy.restore()?;
        session.system_proxy = None;
    }
    selected_backend
        .connect(state, repo, selected, &mut session.backend)
        .await
}
