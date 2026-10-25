//! Engine boundary. Backend selection is centralized; an active session owns its engine.
//! Xray's bundled files and config live in `crate::xray`.
pub mod dns;
pub mod xray;

use crate::{app::state::AppState, domain::errors::Result, storage::repository::Repository};
use std::{future::Future, pin::Pin, sync::Arc, time::Instant};

pub type BackendFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;

/// Result of pushing a catalog change into a core that is already running.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LiveRoutingStatus {
    Applied,
    /// The core is down, or the tunnel mode itself changed. A full connect may proceed.
    NeedsConnect,
    /// The running core kept its tunnel. The catalog change has to be reverted.
    KeepTunnel,
}

pub trait CoreBackend: Send + Sync {
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
    ) -> BackendFuture<'a, LiveRoutingStatus>;
}

pub fn backend() -> &'static dyn CoreBackend {
    &xray::XrayBackend
}

pub type BackendSession = xray::XraySession;

#[derive(Default)]
pub struct Session {
    pub generation: u64,
    pub backend: BackendSession,
    #[cfg(any(windows, target_os = "macos"))]
    pub system_proxy: Option<crate::platform::proxy::SystemProxy>,
}
impl Session {
    pub(crate) fn rollback_repository(&self, repo: &Repository) -> Repository {
        let mut previous = repo.clone();
        previous.settings.network_mode = if self.is_tun() { "tun" } else { "proxy" }.into();
        previous
    }

    pub fn started(&self) -> Option<Instant> {
        self.backend.started
    }
    pub fn proxy_port(&self) -> u16 {
        self.backend.proxy_port
    }
    pub fn http_port(&self) -> u16 {
        self.backend.metrics_port + 1
    }
    pub fn is_tun(&self) -> bool {
        self.backend.is_tun
    }
    pub fn has_system_proxy(&self) -> bool {
        #[cfg(any(windows, target_os = "macos"))]
        {
            self.system_proxy.is_some()
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        {
            false
        }
    }
    /// Process/task liveness is separate from the end-to-end path verification.
    pub fn failed(&mut self) -> bool {
        xray::failed(&mut self.backend)
    }
}

/// The active backend is never inferred from mutable saved settings.
pub async fn connect(
    state: &Arc<AppState>,
    repo: &Repository,
    selected: &str,
    session: &mut Session,
) -> Result<()> {
    let selected_backend = backend();
    selected_backend.validate(repo, selected)?;
    // An Xray system-proxy session may switch live on the same local port; the
    // caller replaces the Windows proxy only when the listener changes.
    #[cfg(any(windows, target_os = "macos"))]
    if repo.settings.network_mode == "tun" {
        if let Some(proxy) = session.system_proxy.as_ref() {
            proxy.restore()?;
            session.system_proxy = None;
        }
    }
    selected_backend
        .connect(state, repo, selected, &mut session.backend)
        .await
}
