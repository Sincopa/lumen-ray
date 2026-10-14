use crate::{
    domain::errors::Result,
    services::vpn_service::Session,
    storage::repository::{Repository, RepositoryStore},
};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::{Mutex, RwLock};
pub struct AppState {
    pub data: PathBuf,
    pub core: RwLock<PathBuf>,
    pub updates: Mutex<()>,
    pub repository: RwLock<Repository>,
    pub store: RepositoryStore,
    pub mutations: Mutex<()>,
    pub session: Mutex<Session>,
    pub connection_operation: tokio::sync::Semaphore,
    pub snapshot: RwLock<crate::services::vpn_service::Snapshot>,
    pub shutdown: tokio::sync::watch::Sender<bool>,
    pub traffic: RwLock<crate::services::stats_service::TrafficStats>,
    pub latencies: RwLock<std::collections::HashMap<String, serde_json::Value>>,
    pub desired_server: RwLock<Option<String>>,
    pub recovery_running: std::sync::atomic::AtomicBool,
    pub recovery_attempts: Mutex<std::collections::VecDeque<std::time::Instant>>,
    pub connection_generation: std::sync::atomic::AtomicU64,
    pub server_quality:
        RwLock<std::collections::HashMap<String, crate::services::server_quality::Quality>>,
    pub device_identity: Arc<crate::platform::device_identity::DeviceIdentityManager>,
    pub subscription_client: Arc<crate::subscription::fetcher::SubscriptionClient>,
    _instance: std::fs::File,
}
impl AppState {
    pub fn open(
        data: PathBuf,
        resources: impl Into<crate::runtime::RuntimePaths>,
    ) -> Result<Arc<Self>> {
        use crate::domain::errors::BackendError;
        std::fs::create_dir_all(&data).map_err(|_| {
            BackendError::new("STORAGE_FAILED", "Cannot create application directory")
        })?;
        let instance = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(data.join("instance.lock"))
            .map_err(|_| BackendError::new("STORAGE_FAILED", "Cannot open application lock"))?;
        fs2::FileExt::try_lock_exclusive(&instance)
            .map_err(|_| BackendError::new("ALREADY_RUNNING", "Lumen is already running"))?;
        let store = RepositoryStore::new(&data.join("data"))?;
        let mut repository = store.load()?;
        if crate::services::catalog_service::upgrade_russia_optimized(&mut repository) {
            log::info!(
                "Updated the Russia preset: blocked services and Discord use the VPN, other traffic stays direct"
            );
            if let Err(error) = store.save(&repository) {
                log::warn!("Could not save the updated routing preset: {}", error.code);
            }
        }
        // Automatic reconnection is now on by default. Existing data had it off
        // only because that was the old default, which left a dropped tunnel
        // disconnected for good. Apply once; a later opt-out is kept.
        let reconnect_marker = data.join("data").join("auto-reconnect-default");
        if !reconnect_marker.exists() {
            let saved = if repository.settings.auto_reconnect {
                Ok(())
            } else {
                repository.settings.auto_reconnect = true;
                store.save(&repository)
            };
            match saved {
                Ok(()) => {
                    let _ = std::fs::write(&reconnect_marker, b"1");
                }
                Err(error) => {
                    log::warn!("Could not enable automatic reconnection: {}", error.code);
                }
            }
        }
        #[cfg(windows)]
        crate::platform::proxy::SystemProxy::recover(&data)?;
        let core = crate::xray::core::prepare(resources.into(), &data)?;
        let device_identity = Arc::new(
            crate::platform::device_identity::DeviceIdentityManager::new(
                data.join("identity.dpapi"),
            ),
        );
        if let Err(e) = device_identity.get_or_init() {
            log::warn!("Device identity initialization warning: {:?}", e);
        }
        let subscription_client = Arc::new(crate::subscription::fetcher::SubscriptionClient::new(
            Arc::clone(&device_identity)
                as Arc<dyn crate::subscription::fetcher::DeviceIdentityProvider>,
        ));
        let (shutdown, _) = tokio::sync::watch::channel(false);
        Ok(Arc::new(Self {
            data,
            core: RwLock::new(core),
            updates: Mutex::new(()),
            repository: RwLock::new(repository),
            store,
            mutations: Mutex::new(()),
            session: Mutex::new(Session::default()),
            connection_operation: tokio::sync::Semaphore::new(1),
            snapshot: RwLock::new(Default::default()),
            shutdown,
            traffic: RwLock::new(Default::default()),
            latencies: RwLock::new(std::collections::HashMap::new()),
            desired_server: RwLock::new(None),
            recovery_running: std::sync::atomic::AtomicBool::new(false),
            recovery_attempts: Mutex::new(Default::default()),
            connection_generation: std::sync::atomic::AtomicU64::new(0),
            server_quality: RwLock::new(Default::default()),
            device_identity,
            subscription_client,
            _instance: instance,
        }))
    }
}
