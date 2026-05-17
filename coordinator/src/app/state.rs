use alloc::sync::Arc;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use tokio::time::Instant;

use eyre::WrapErr as _;
use serde::{Deserialize, Serialize};
use shuthost_common::protocol::{InitSystem, OsType};
use tokio::sync::{RwLock, broadcast, watch};
use tracing::info;
use web_push_native::jwt_simple::algorithms::ES256KeyPair;

use super::shared_watch_store::SharedWatchStore;
use crate::{
    app::{
        LeaseMapRaw,
        db::{self, DbPool},
        host_actor::HostActorHandle,
        host_control::LeaseStore,
    },
    config::{
        ControllerConfig, DbConfig, RuntimeConfig, TlsConfig, load, resolve_config_relative_paths,
    },
    http::{EXPECTED_AUTH_EXCEPTIONS_VERSION, auth},
    websocket::WsMessage,
};

/// Host online/offline state, including active transition states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostState {
    Online,
    Offline,
    /// Wake-on-LAN sent; waiting for the host to respond.
    Waking,
    /// Shutdown command sent; waiting for the host to stop responding.
    ShuttingDown,
}

impl HostState {
    /// Returns `true` for `Waking` and `ShuttingDown`.
    pub(crate) const fn is_transitioning(self) -> bool {
        matches!(self, Self::Waking | Self::ShuttingDown)
    }
}

pub(crate) type ConfigRx = watch::Receiver<Arc<ControllerConfig>>;
pub(super) type ConfigTx = watch::Sender<Arc<ControllerConfig>>;
pub type HostStatus = HashMap<String, HostState>;
/// Alias kept for compatibility with existing watch subscribers.
pub(crate) type HostStatusRx = watch::Receiver<Arc<HostStatus>>;
pub(crate) type OperationFailureStore = SharedWatchStore<OperationFailureMap>;
pub(crate) type WsTx = broadcast::Sender<WsMessage>;

/// The kind of control operation that failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OperationKind {
    Shutdown,
    Startup,
}

/// Records the last failed control operation for a host.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationFailure {
    pub operation: OperationKind,
}

/// Map of `host_name → OperationFailure` for hosts whose last operation failed.
pub type OperationFailureMap = HashMap<String, OperationFailure>;

impl SharedWatchStore<OperationFailureMap> {
    /// Record a failure for `host`.
    ///
    /// Returns `true` if this is a new failure or the operation kind changed
    /// (i.e. a push notification should be sent). Returns `false` if the
    /// identical failure was already recorded, suppressing repeated notifications
    /// for hosts that are retried periodically.
    pub(crate) async fn set(&self, host: &str, failure: OperationFailure) -> bool {
        let mut inner = self.inner.lock().await;
        let is_new = inner
            .get(host)
            .is_none_or(|existing| existing.operation != failure.operation);
        inner.insert(host.to_string(), failure);
        drop(self.tx.send(Arc::new(inner.clone())));
        is_new
    }

    /// Clear any recorded failure for `host` (e.g. on a successful operation).
    pub(crate) async fn clear(&self, host: &str) {
        let mut inner = self.inner.lock().await;
        if inner.remove(host).is_some() {
            drop(self.tx.send(Arc::new(inner.clone())));
        }
    }
}

/// Cached install metadata for a host, populated from the DB on startup
/// and updated live when agent startup broadcasts arrive.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct HostInstallInfo {
    pub agent_version: Option<String>,
    pub init_system: Option<InitSystem>,
    pub os: Option<OsType>,
    pub script_path: Option<String>,
}

/// Application state shared across request handlers and background tasks.
#[derive(Clone)]
pub(crate) struct AppState {
    /// Path to the configuration file for template injection and reloads.
    pub config_path: PathBuf,

    /// Receiver for updated `ControllerConfig` when the file changes.
    pub config_rx: ConfigRx,

    /// Single-owner host state machine actor.
    pub host_actor: HostActorHandle,

    /// Broadcast sender for distributing WebSocket messages.
    pub ws_tx: WsTx,

    /// In-memory map of current leases for hosts (write-serialized, watch-observable).
    pub leases: Arc<LeaseStore>,

    /// Runtime IP/port overrides for hosts whose address differs from the static config.
    /// Populated from the DB on startup and updated live when agent startup broadcasts arrive.
    pub host_overrides: Arc<RwLock<HashMap<String, db::HostOverride>>>,

    /// Cached known agent install info from the DB and runtime events.
    pub host_install_info: Arc<RwLock<HashMap<String, HostInstallInfo>>>,

    /// Authentication runtime (mode and secrets)
    pub auth: Arc<auth::Runtime>,
    /// Whether the HTTP server was started with TLS enabled (true for HTTPS)
    pub tls_enabled: bool,

    /// Runtime tuning parameters (poll intervals, default timeouts, etc.).
    /// Snapshotted at startup; a restart is required to apply changes.
    pub runtime: RuntimeConfig,

    /// Database connection pool for persistent storage.
    pub db_pool: Option<DbPool>,

    /// VAPID key builder for signing web push notifications.
    /// `None` when DB persistence is disabled.
    pub vapid_key: Option<Arc<ES256KeyPair>>,

    /// Per-host record of the last failed control operation (ephemeral, not persisted).
    pub operation_failures: Arc<OperationFailureStore>,

    /// Tracks when each host most recently transitioned to Online (ephemeral, not persisted).
    /// Used to validate deferred online-for notifications — if the `Instant` at notification
    /// time matches the one recorded at subscribe time, the host is still in the same online
    /// session.
    pub online_since: Arc<RwLock<HashMap<String, Instant>>>,
}

/// Initialize database pool based on configuration.
///
/// Initialize database. If a persistent DB is configured and enabled, open it
/// relative to the config file when appropriate. Otherwise DB persistence is
/// disabled and `db_pool` will be None.
#[tracing::instrument(skip_all)]
async fn initialize_database(
    initial_config: &ControllerConfig,
    config_path: &Path,
) -> eyre::Result<Option<DbPool>> {
    Ok(match initial_config.db {
        Some(DbConfig {
            enable: true,
            ref path,
        }) => {
            let db_path = resolve_config_relative_paths(config_path, path);
            let pool = db::init(&db_path).await.wrap_err(format!(
                "Failed to initialize database at: {}",
                db_path.display()
            ))?;
            info!(
                "Database initialized at: {} (note: WAL mode creates .db-wal and .db-shm files alongside)",
                db_path.display()
            );
            Some(pool)
        }
        _ => {
            info!("DB persistence disabled");
            None
        }
    })
}

// TODO: consider showing warning in gui as well
pub fn emit_warning_on_unsaved_sync_state(app_state: &ControllerConfig) {
    if !matches!(app_state.db, Some(DbConfig { enable: true, .. })) {
        let has_enforcing_hosts: Vec<_> = app_state
            .hosts
            .iter()
            .filter(|&(_, h)| h.enforce_state)
            .map(|(n, _)| n.clone())
            .collect();
        if !has_enforcing_hosts.is_empty() {
            let host_names = has_enforcing_hosts.join(", ");
            tracing::warn!(
                "Database persistence is disabled but there are hosts with enforce_state=true ({host_names}). The coordinator will lose all lease state on restarts or updates, potentially causing these hosts to be shut down unexpectedly."
            );
        }
    }
}

/// Emit startup warnings based on configuration and runtime state.
fn emit_startup_warnings(app_state: &AppState, app_config: &ControllerConfig) {
    #[cfg(unix)]
    {
        use std::fs;
        use std::os::unix::fs::PermissionsExt as _;
        if let Ok(metadata) = fs::metadata(&app_state.config_path) {
            let mode = metadata.permissions().mode();
            if mode & 0o077 != 0 {
                tracing::warn!(
                    "Config file permissions are too permissive (current: {mode:#o}). Run 'chmod 600 {}' to restrict access to owner only.",
                    app_state.config_path.display()
                );
            }
        }
    }

    if !app_state.tls_enabled {
        match &app_state.auth.mode {
            &auth::Resolved::Disabled => {}
            _ => {
                tracing::warn!(
                    "TLS appears disabled but authentication is enabled. Authentication cookies are set with Secure=true and will not be sent by browsers over plain HTTP. Enable TLS or run behind an HTTPS reverse proxy (ensure it sets X-Forwarded-Proto: https)."
                );
            }
        }
    }

    match &app_state.auth.mode {
        &auth::Resolved::External { exceptions_version }
            if exceptions_version != EXPECTED_AUTH_EXCEPTIONS_VERSION =>
        {
            tracing::warn!(
                "External authentication is configured with an outdated exceptions version ({exceptions_version}, current {}).",
                EXPECTED_AUTH_EXCEPTIONS_VERSION
            );
        }
        _ => {}
    }

    emit_warning_on_unsaved_sync_state(app_config);
}

async fn load_leases(db_pool: Option<&DbPool>) -> eyre::Result<Arc<LeaseStore>> {
    let mut initial_leases = LeaseMapRaw::default();
    if let Some(pool) = db_pool {
        db::load_leases(pool, &mut initial_leases).await?;
        info!("Loaded leases from database");
    } else {
        info!("Skipping lease load: DB persistence disabled");
    }
    let (leases, _) = LeaseStore::new(initial_leases);
    Ok(leases)
}

async fn load_host_overrides(
    db_pool: Option<&DbPool>,
    initial_config: &ControllerConfig,
) -> eyre::Result<Arc<RwLock<HashMap<String, db::HostOverride>>>> {
    let overrides = if let Some(pool) = db_pool {
        let overrides = db::load_host_ip_overrides(pool).await?;
        for (name, o) in &overrides {
            if let Some(h) = initial_config.hosts.get(name)
                && (h.ip != o.ip || h.port != o.port)
            {
                tracing::warn!(
                    "Host '{name}' has a stored IP/port override: config={}:{}, stored={}:{}",
                    h.ip,
                    h.port,
                    o.ip,
                    o.port
                );
            }
        }
        overrides
    } else {
        HashMap::default()
    };
    Ok(Arc::new(RwLock::new(overrides)))
}

async fn load_host_install_info(
    db_pool: Option<&DbPool>,
) -> eyre::Result<Arc<RwLock<HashMap<String, HostInstallInfo>>>> {
    let host_install_info = if let Some(pool) = db_pool {
        let host_stats = db::get_all_host_stats(pool).await?;
        Arc::new(RwLock::new(
            host_stats
                .into_iter()
                .map(|(hostname, stats)| {
                    (
                        hostname,
                        HostInstallInfo {
                            agent_version: stats.agent_version,
                            init_system: stats.init_system,
                            os: stats.operating_system,
                            script_path: stats.script_path,
                        },
                    )
                })
                .collect(),
        ))
    } else {
        Arc::default()
    };
    Ok(host_install_info)
}

async fn load_vapid_key(db_pool: Option<&DbPool>) -> eyre::Result<Option<Arc<ES256KeyPair>>> {
    if let Some(pool) = db_pool {
        let pem = match db::get_kv(pool, db::KV_VAPID_PRIVATE_KEY_PEM).await? {
            Some(pem) => pem,
            None => {
                let key = rcgen::KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256)
                    .wrap_err("Failed to generate VAPID EC key")?;
                let pem = key.serialize_pem();
                db::store_kv(pool, db::KV_VAPID_PRIVATE_KEY_PEM, &pem).await?;
                info!("Generated and stored new VAPID private key");
                pem
            }
        };
        Ok(Some(Arc::new(ES256KeyPair::from_pem(&pem).map_err(
            |e| eyre::eyre!("Failed to load VAPID private key from PEM: {e:?}"),
        )?)))
    } else {
        info!("VAPID key unavailable: DB persistence disabled");
        Ok(None)
    }
}
/// Initialize application state and start background tasks.
#[tracing::instrument(skip_all)]
pub(super) async fn initialize_state(
    config_path: &Path,
) -> eyre::Result<(AppState, Option<TlsConfig>, ConfigTx)> {
    let initial_config = Arc::new(load(config_path).await?);

    let (config_tx, config_rx) = watch::channel(initial_config.clone());
    let host_actor = HostActorHandle::spawn(HostStatus::new());
    let (ws_tx, _) = broadcast::channel(32);
    let (operation_failures, _) = OperationFailureStore::new(OperationFailureMap::new());

    let db_pool = initialize_database(&initial_config, config_path).await?;
    let leases = load_leases(db_pool.as_ref()).await?;
    let host_overrides = load_host_overrides(db_pool.as_ref(), &initial_config).await?;
    let host_install_info = load_host_install_info(db_pool.as_ref()).await?;

    let auth_runtime =
        Arc::new(auth::Runtime::from_config(&initial_config.server.auth, db_pool.as_ref()).await?);

    let tls_opt = match initial_config.server.tls {
        Some(ref tls_cfg @ TlsConfig { enable: true, .. }) => Some(tls_cfg.clone()),
        _ => None,
    };

    let vapid_key = load_vapid_key(db_pool.as_ref()).await?;

    let app_state = AppState {
        config_rx,
        host_actor,
        ws_tx,
        config_path: config_path.to_path_buf(),
        leases,
        host_overrides,
        host_install_info,
        auth: auth_runtime.clone(),
        tls_enabled: tls_opt.is_some(),
        runtime: initial_config.server.runtime.clone(),
        db_pool,
        vapid_key,
        operation_failures,
        online_since: Arc::new(RwLock::new(HashMap::new())),
    };

    emit_startup_warnings(&app_state, &initial_config);

    Ok((app_state, tls_opt, config_tx))
}
