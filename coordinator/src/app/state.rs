use alloc::sync::Arc;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use eyre::WrapErr as _;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, watch};
use tracing::info;

use crate::{
    app::{
        LeaseMapRaw, LeaseState,
        db::{self, DbPool},
        runtime::start_background_tasks,
    },
    config::{ControllerConfig, DbConfig, TlsConfig, load, resolve_config_relative_paths},
    http::{EXPECTED_AUTH_EXCEPTIONS_VERSION, auth},
    websocket::WsMessage,
};

/// Host online/offline state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HostState {
    Online,
    Offline,
}

pub(crate) type ConfigRx = watch::Receiver<Arc<ControllerConfig>>;
pub(super) type ConfigTx = watch::Sender<Arc<ControllerConfig>>;
pub type HostStatus = HashMap<String, HostState>;
pub(crate) type HostStatusRx = watch::Receiver<Arc<HostStatus>>;
pub(crate) type HostStatusTx = watch::Sender<Arc<HostStatus>>;
pub(crate) type WsTx = broadcast::Sender<WsMessage>;

/// Application state shared across request handlers and background tasks.
#[derive(Clone)]
pub(crate) struct AppState {
    /// Path to the configuration file for template injection and reloads.
    pub config_path: PathBuf,

    /// Receiver for updated `ControllerConfig` when the file changes.
    pub config_rx: ConfigRx,

    /// Receiver for host online/offline status updates.
    pub hoststatus_rx: HostStatusRx,
    /// Sender for host online/offline status updates.
    pub hoststatus_tx: HostStatusTx,

    /// Broadcast sender for distributing WebSocket messages.
    pub ws_tx: WsTx,

    /// In-memory map of current leases for hosts (write-serialized, watch-observable).
    pub leases: Arc<LeaseState>,

    /// Authentication runtime (mode and secrets)
    pub auth: Arc<auth::Runtime>,
    /// Whether the HTTP server was started with TLS enabled (true for HTTPS)
    pub tls_enabled: bool,

    /// Database connection pool for persistent storage.
    pub db_pool: Option<DbPool>,
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

/// Emit startup warnings based on configuration and runtime state.
fn emit_startup_warnings(app_state: &AppState) {
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
}

/// Initialize application state and start background tasks.
#[tracing::instrument(skip_all)]
pub(super) async fn initialize_state(
    config_path: &Path,
) -> eyre::Result<(AppState, Option<TlsConfig>)> {
    let initial_config = Arc::new(load(config_path).await?);

    let (config_tx, config_rx) = watch::channel(initial_config.clone());

    let initial_status = Arc::new(HostStatus::new());
    let (hoststatus_tx, hoststatus_rx) = watch::channel(initial_status);

    let (ws_tx, _) = broadcast::channel(32);

    let db_pool = initialize_database(&initial_config, config_path).await?;

    let mut initial_leases = LeaseMapRaw::default();

    if let Some(ref pool) = db_pool {
        db::load_leases(pool, &mut initial_leases).await?;
        info!("Loaded leases from database");
    } else {
        info!("Skipping lease load: DB persistence disabled");
    }

    let (leases, _) = LeaseState::new(initial_leases);

    let auth_runtime =
        Arc::new(auth::Runtime::from_config(&initial_config.server.auth, db_pool.as_ref()).await?);

    let tls_opt = match initial_config.server.tls {
        Some(ref tls_cfg @ TlsConfig { enable: true, .. }) => Some(tls_cfg.clone()),
        _ => None,
    };

    let app_state = AppState {
        config_rx,
        hoststatus_rx,
        hoststatus_tx,
        ws_tx,
        config_path: config_path.to_path_buf(),
        leases,
        auth: auth_runtime.clone(),
        tls_enabled: tls_opt.is_some(),
        db_pool,
    };

    // Start background tasks now that the full AppState is available.
    start_background_tasks(&app_state, &config_tx, config_path);

    emit_startup_warnings(&app_state);

    Ok((app_state, tls_opt))
}
