//! Native local proxy milestone. Unsupported TUN, UDP and protocol profiles fail closed.
mod dns;
pub(crate) mod outbound;
mod proxy;
mod routing;
#[cfg(test)]
mod tests;

use super::{BackendFuture, BackendSession, CoreBackend, CoreCapabilities, CoreEngine};
use crate::{
    app::state::AppState,
    domain::{
        errors::{BackendError, Result},
        node::{Protocol, VpnNode},
    },
    storage::repository::Repository,
};
use dns::DnsConfig;
use routing::RoutingTable;
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Instant,
};
use tokio::{net::TcpListener, sync::RwLock, task::JoinHandle};

#[derive(Default)]
pub struct Counters {
    upload: AtomicU64,
    download: AtomicU64,
}
impl Counters {
    pub fn read(&self) -> (u64, u64) {
        (
            self.upload.load(Ordering::Relaxed),
            self.download.load(Ordering::Relaxed),
        )
    }
}

pub struct NativeSession {
    pub started: Instant,
    pub port: u16,
    pub counters: Arc<Counters>,
    routing: Arc<RwLock<Arc<RoutingTable>>>,
    stop: tokio::sync::watch::Sender<bool>,
    listener: JoinHandle<()>,
}
impl NativeSession {
    pub fn finished(&self) -> bool {
        self.listener.is_finished()
    }
    pub(crate) async fn shutdown(mut self) {
        let _ = self.stop.send(true);
        let _ = (&mut self.listener).await;
    }
}
impl Drop for NativeSession {
    fn drop(&mut self) {
        self.listener.abort();
    }
}
pub struct LumenBackend;
pub fn unsupported() -> BackendError {
    BackendError::new("LUMEN_UNSUPPORTED_CONFIGURATION", "This configuration is not supported by Lumen Core yet. Use Xray Core for this server or mode.")
}
fn validate_node(node: &VpnNode) -> Result<()> {
    node.validate()?;
    if !matches!(node.protocol, Protocol::Socks | Protocol::Http)
        || !matches!(node.transport.network.as_str(), "raw" | "tcp")
        || node.security.kind != "none"
        || !node.flow.is_empty()
        || node.balancer.is_some()
        || node.wireguard.is_some()
        || !node.transport.path.is_empty()
        || !node.transport.host.is_empty()
        || !node.transport.service_name.is_empty()
        || !node.transport.mode.is_empty()
        || !node.transport.seed.is_empty()
        || !matches!(node.transport.header_type.as_str(), "" | "none")
        || node.method.is_empty() != node.credential.is_empty()
        || (node.protocol == Protocol::Socks
            && (node.method.len() > 255 || node.credential.len() > 255))
        || node.method.contains(':')
    {
        return Err(unsupported());
    }
    Ok(())
}
impl LumenBackend {
    fn prepare(repo: &Repository, selected: &str) -> Result<(RoutingTable, DnsConfig)> {
        if repo.settings.network_mode != "proxy" || repo.settings.dns_policy.enabled {
            return Err(unsupported());
        }
        let dns = if repo.settings.dns_server == "system" {
            DnsConfig::System
        } else {
            DnsConfig::Tcp(
                repo.settings
                    .dns_server
                    .parse()
                    .map_err(|_| unsupported())?,
            )
        };
        Ok((RoutingTable::compile(repo, selected)?, dns))
    }
    pub async fn start_native(repo: &Repository, selected: &str) -> Result<NativeSession> {
        let (routing, dns) = Self::prepare(repo, selected)?;
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .map_err(|_| BackendError::new("LUMEN_START_FAILED", "Cannot bind local proxy"))?;
        let port = listener
            .local_addr()
            .map_err(|_| BackendError::new("LUMEN_START_FAILED", "Cannot inspect local proxy"))?
            .port();
        let counters = Arc::new(Counters::default());
        let routing = Arc::new(RwLock::new(Arc::new(routing)));
        let (stop, stopped) = tokio::sync::watch::channel(false);
        let task = tokio::spawn(proxy::serve(
            listener,
            routing.clone(),
            dns,
            counters.clone(),
            stopped,
        ));
        Ok(NativeSession {
            started: Instant::now(),
            port,
            counters,
            routing,
            stop,
            listener: task,
        })
    }
}
impl CoreBackend for LumenBackend {
    fn capabilities(&self) -> CoreCapabilities {
        CoreCapabilities {
            engine: CoreEngine::Lumen,
            experimental: true,
            tun: false,
            system_proxy: true,
            udp: false,
            live_routing_updates: true,
            protocols: vec!["socks", "http"],
            geosite: false,
            geoip: false,
            application_routing: false,
            reality: false,
            xtls_vision: false,
            kill_switch: false,
        }
    }
    fn validate(&self, repo: &Repository, selected: &str) -> Result<()> {
        Self::prepare(repo, selected).map(|_| ())
    }
    fn connect<'a>(
        &self,
        _state: &'a Arc<AppState>,
        repo: &'a Repository,
        selected: &'a str,
        session: &'a mut BackendSession,
    ) -> BackendFuture<'a, ()> {
        Box::pin(async move {
            let BackendSession::Lumen(s) = session else {
                return Err(unsupported());
            };
            Self::prepare(repo, selected)?;
            if let Some(old) = s.take() {
                old.shutdown().await;
            }
            *s = Some(Self::start_native(repo, selected).await?);
            log::info!("engine=lumen native TCP proxy started");
            Ok(())
        })
    }
    fn prewarm<'a>(
        &self,
        _state: &'a Arc<AppState>,
        _repo: &'a Repository,
        _selected: Option<&'a str>,
        _session: &'a mut BackendSession,
    ) -> BackendFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
    fn disconnect<'a>(
        &self,
        session: &'a mut BackendSession,
        _keep_warm: bool,
    ) -> BackendFuture<'a, ()> {
        Box::pin(async move {
            if let BackendSession::Lumen(s) = session {
                if let Some(s) = s.take() {
                    s.shutdown().await;
                }
            }
            Ok(())
        })
    }
    fn update_routing<'a>(
        &self,
        _state: &'a Arc<AppState>,
        selected: &'a str,
        before: &'a Repository,
        after: &'a Repository,
        session: &'a mut BackendSession,
    ) -> BackendFuture<'a, crate::engine::LiveRoutingStatus> {
        Box::pin(async move {
            let BackendSession::Lumen(Some(s)) = session else {
                return Ok(crate::engine::LiveRoutingStatus::NeedsConnect);
            };
            if before.nodes != after.nodes
                || before.settings.network_mode != after.settings.network_mode
                || before.settings.dns_server != after.settings.dns_server
                || before.settings.dns_policy != after.settings.dns_policy
            {
                return Ok(crate::engine::LiveRoutingStatus::NeedsConnect);
            }
            let (table, _) = Self::prepare(after, selected)?;
            *s.routing.write().await = Arc::new(table);
            Ok(crate::engine::LiveRoutingStatus::Applied)
        })
    }
}
