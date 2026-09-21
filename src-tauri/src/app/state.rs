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
    _instance: std::fs::File,
}
impl AppState {
    pub fn open(data: PathBuf, resources: PathBuf) -> Result<Arc<Self>> {
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
        let repository = store.load()?;
        #[cfg(windows)]
        crate::platform::proxy::SystemProxy::recover(&data)?;
        let core = crate::xray::core::prepare(&resources, &data)?;
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
            _instance: instance,
        }))
    }
}
