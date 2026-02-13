//! HTTP server implementation for the coordinator control interface.
//!
//! Defines routes, state management, configuration watching, and server startup.

use alloc::{string, sync::Arc};
use core::{
    net::{IpAddr, SocketAddr},
    time::Duration,
};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use axum::{
    Router,
    body::Body,
    http::{
        HeaderValue, Request, StatusCode,
        header::{AUTHORIZATION, COOKIE, HeaderName},
    },
    middleware::{self as ax_middleware, Next},
    response::{Redirect, Response},
    routing::{self, IntoMakeService, any, get},
};
use axum_server::tls_rustls::RustlsConfig as AxumRustlsConfig;
use eyre::{WrapErr as _, eyre};
use secrecy::{ExposeSecret as _, SecretBox};
use tokio::{
    fs as t_fs, net, signal,
    sync::{broadcast, watch},
};
use tower::ServiceBuilder;
use tower_http::{ServiceBuilderExt as _, request_id::MakeRequestUuid, timeout::TimeoutLayer};
use tracing::{info, warn};

use crate::{
    auth,
    config::{ControllerConfig, DbConfig, TlsConfig, load, resolve_config_relative_paths},
    db::{self, DbPool},
    http::{
        api,
        assets::{self, serve_ui},
        download, login,
        m2m::{self, LeaseMap},
        polling,
    },
    websocket::{WsMessage, ws_handler},
};

/// Version number for validating external authentication exceptions.
///
/// This constant ensures compatibility with external authentication systems by checking
/// the exceptions version against expected values. It is used in authentication resolution
/// logic to validate external auth modes.
///
/// It is interdependent with the [`create_app_router`] function in this module, as the public routes
/// defined there include authentication endpoints (e.g., login, logout, OIDC callbacks) whose behavior and
/// accessibility may depend on this version when handling external authentication modes.
/// When routes get added to public routes, this needs to be bumped.
pub(crate) const EXPECTED_AUTH_EXCEPTIONS_VERSION: u32 = 2;

/// Creates the main application router by merging public and private routes.
///
/// Public routes include authentication endpoints (login, logout, OIDC), static assets,
/// downloads, and M2M APIs that are accessible without authentication.
/// Private routes include the main UI, API endpoints, and WebSocket handler, protected by auth middleware.
///
/// When routes get added to public routes, [`EXPECTED_AUTH_EXCEPTIONS_VERSION`] needs to be bumped.
fn create_app_router(auth_runtime: &Arc<auth::Runtime>) -> Router<AppState> {
    let public = Router::new()
        .merge(login::routes())
        // PWA & static assets bundled via asset_routes
        .merge(assets::routes())
        // Bypass routes
        .nest("/download", download::routes())
        .nest("/api/m2m", m2m::routes());

    let private = Router::new()
        .nest("/api", api::routes())
        .route("/", get(serve_ui))
        .route("/ws", any(ws_handler))
        .route_layer(ax_middleware::from_fn_with_state(
            auth::LayerState {
                auth: auth_runtime.clone(),
            },
            auth::require,
        ));

    public.merge(private)
}

pub(crate) type ConfigRx = watch::Receiver<Arc<ControllerConfig>>;
pub(crate) type ConfigTx = watch::Sender<Arc<ControllerConfig>>;
/// Maps hostname to online status (true=online, false=offline)
pub(crate) type HostStatus = HashMap<String, bool>;
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

    /// In-memory map of current leases for hosts.
    pub leases: LeaseMap,

    /// Authentication runtime (mode and secrets)
    pub auth: Arc<auth::Runtime>,
    /// Whether the HTTP server was started with TLS enabled (true for HTTPS)
    pub tls_enabled: bool,

    /// Database connection pool for persistent storage.
    pub db_pool: Option<DbPool>,
}

/// Initialize database pool based on configuration.
async fn initialize_database(
    initial_config: &ControllerConfig,
    config_path: &Path,
) -> eyre::Result<Option<DbPool>> {
    // Initialize database. If a persistent DB is configured and enabled, open it
    // relative to the config file when appropriate. Otherwise DB persistence is
    // disabled and `db_pool` will be None.
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

/// Setup TLS configuration for HTTPS server.
async fn setup_tls_config(
    tls_cfg: &TlsConfig,
    config_path: &Path,
    listen_ip: IpAddr,
    addr: SocketAddr,
) -> eyre::Result<AxumRustlsConfig> {
    // Use provided certs when both files exist. Otherwise, if persist_self_signed is true
    // (default), generate and persist self-signed cert/key next to the config file.
    let cert_path_cfg = tls_cfg.cert_path.as_str();
    let key_path_cfg = tls_cfg.key_path.as_str();

    let cert_path = resolve_config_relative_paths(config_path, cert_path_cfg);
    let key_path = resolve_config_relative_paths(config_path, key_path_cfg);

    let cert_exists = cert_path.exists();
    let key_exists = key_path.exists();

    let rustls_cfg = if cert_exists && key_exists {
        let rustls_cfg = AxumRustlsConfig::from_pem_file(
            cert_path
                .to_str()
                .ok_or_else(|| eyre!("Invalid Cert-Path"))?,
            key_path.to_str().ok_or_else(|| eyre!("Invalid Key-Path"))?,
        )
        .await
        .wrap_err(format!(
            "Failed to load TLS certificates from cert: {}, key: {}",
            cert_path.display(),
            key_path.display()
        ))?;
        info!("Listening on https://{} (provided certs)", addr);
        rustls_cfg
    } else if tls_cfg.persist_self_signed {
        // If cert files already exist partially, refuse to do anything.
        if cert_exists ^ key_exists {
            eyre::bail!("TLS configuration error: partial cert/key files exist");
        }

        // Generate self-signed cert using listen host as CN/SAN
        let hostnames = vec![listen_ip.to_string()];
        let rcgen::CertifiedKey { cert, signing_key } =
            rcgen::generate_simple_self_signed(hostnames)
                .wrap_err("Failed to generate self-signed certificate")?;
        let cert_pem = cert.pem();
        let key_pem = SecretBox::new(Box::new(signing_key.serialize_pem().into_bytes()));

        // Ensure parent dir exists (typically same dir as config)
        let cfg_dir = config_path.parent().unwrap_or_else(|| Path::new("."));
        t_fs::create_dir_all(&cfg_dir).await.wrap_err(format!(
            "Failed to create certificate directory at: {}",
            cfg_dir.display()
        ))?;

        // Write cert/key files
        tokio::try_join!(
            t_fs::write(&cert_path, cert_pem.as_bytes()),
            t_fs::write(&key_path, key_pem.expose_secret())
        )
        .wrap_err(format!(
            "Failed to write TLS certificates to cert: {}, key: {}",
            cert_path.display(),
            key_path.display()
        ))?;

        let rustls_cfg =
            AxumRustlsConfig::from_pem(cert_pem.into_bytes(), key_pem.expose_secret().clone())
                .await?;
        info!(
            "Listening on https://{} (self-signed, persisted at {:?})",
            addr, cfg_dir
        );
        rustls_cfg
    } else {
        eyre::bail!("TLS configuration error: neither provided certs nor self-signed allowed");
    };

    Ok(rustls_cfg)
}

fn create_app(app_state: AppState) -> IntoMakeService<Router<()>> {
    // TODO: figure out rate limiting
    let middleware_stack = ServiceBuilder::new()
        .sensitive_headers([AUTHORIZATION, COOKIE])
        .set_x_request_id(MakeRequestUuid)
        .propagate_x_request_id()
        // must be after request-id
        .trace_for_http();

    #[cfg(any(
        feature = "compression-br",
        feature = "compression-deflate",
        feature = "compression-gzip",
        feature = "compression-zstd",
    ))]
    let middleware_stack = middleware_stack.compression();

    let middleware_stack = middleware_stack
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(30),
        ))
        .layer(ax_middleware::from_fn(secure_headers_middleware));

    // Public routes (login, oidc callback, m2m endpoints, static assets such as PWA manifest, downloads for agent and client installs) must be reachable without auth
    // Private app routes protected by auth middleware
    let app = create_app_router(&app_state.auth)
        .with_state(app_state)
        .fallback(routing::any(|req: Request<Body>| async move {
            tracing::warn!(
                method = %req.method(),
                uri = %req.uri(),
                "Unhandled request"
            );
            Redirect::permanent("/")
        }))
        .layer(middleware_stack);

    app.into_make_service()
}

/// Emit startup warnings based on configuration and runtime state.
fn emit_startup_warnings(app_state: &AppState) {
    // Check config file permissions on Unix systems
    #[cfg(unix)]
    {
        use std::fs;
        use std::os::unix::fs::PermissionsExt as _;
        if let Ok(metadata) = fs::metadata(&app_state.config_path) {
            let mode = metadata.permissions().mode();
            if mode & 0o077 != 0 {
                warn!(
                    "Config file permissions are too permissive (current: {mode:#o}). Run 'chmod 600 {}' to restrict access to owner only.",
                    app_state.config_path.display()
                );
            }
        }
    }

    // Startup-time warning: if TLS is not enabled but authentication is active,
    // browsers will ignore cookies marked Secure. Warn operators so they can
    // enable TLS or place the app behind an HTTPS reverse proxy that sets
    // X-Forwarded-Proto: https.
    if !app_state.tls_enabled {
        match &app_state.auth.mode {
            &auth::Resolved::Disabled => {}
            _ => {
                warn!(
                    "TLS appears disabled but authentication is enabled. Authentication cookies are set with Secure=true and will not be sent by browsers over plain HTTP. Enable TLS or run behind an HTTPS reverse proxy (ensure it sets X-Forwarded-Proto: https)."
                );
            }
        }
    }

    // Startup-time warning: if external auth is configured but exceptions version is outdated,
    // the main page will show a security warning. Warn operators to update the config.
    match &app_state.auth.mode {
        &auth::Resolved::External { exceptions_version }
            if exceptions_version != EXPECTED_AUTH_EXCEPTIONS_VERSION =>
        {
            warn!(
                "External authentication is configured with an outdated exceptions version ({exceptions_version}, current {EXPECTED_AUTH_EXCEPTIONS_VERSION}). The main page will display how to configure the correct exceptions.",
            );
        }
        _ => {}
    }
}

/// Starts the Axum-based HTTP server for the coordinator UI and API.
///
/// # Arguments
///
/// * `config_path` - Path to the TOML configuration file.
/// * `port_override` - Optional port to override the config value.
/// * `bind_override` - Optional bind address to override the config value.
///
/// Initialize the application state and background services.
async fn initialize_state(config_path: &Path) -> eyre::Result<(AppState, Option<TlsConfig>)> {
    let initial_config = Arc::new(load(config_path).await?);

    let (config_tx, config_rx) = watch::channel(initial_config.clone());

    let initial_status = Arc::new(HashMap::<String, bool>::new());
    let (hoststatus_tx, hoststatus_rx) = watch::channel(initial_status);

    let (ws_tx, _) = broadcast::channel(32);

    let db_pool = initialize_database(&initial_config, config_path).await?;

    let leases = LeaseMap::default();

    // Load existing leases from database when persistence is enabled
    if let Some(ref pool) = db_pool {
        db::load_leases(pool, &leases).await?;
        info!("Loaded leases from database");
    } else {
        info!("Skipping lease load: DB persistence disabled");
    }

    // Start background tasks
    polling::start_background_tasks(&config_rx, &hoststatus_tx, &ws_tx, &config_tx, config_path);

    let auth_runtime =
        Arc::new(auth::Runtime::from_config(&initial_config.server.auth, db_pool.as_ref()).await?);

    // Startup-time warning: if TLS is not enabled but authentication is active,
    // browsers will ignore cookies marked Secure. Warn operators so they can
    // enable TLS or place the app behind an HTTPS reverse proxy that sets
    // X-Forwarded-Proto: https.
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

    emit_startup_warnings(&app_state);

    Ok((app_state, tls_opt))
}

/// Creates a future that resolves when a shutdown signal is received.
///
/// On Unix systems, listens for SIGTERM. On other systems, listens for Ctrl+C.
pub(crate) async fn shutdown_signal() {
    #[cfg(unix)]
    {
        let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to create SIGTERM signal handler");
        let _ = sigterm.recv().await;
    }
    #[cfg(not(unix))]
    {
        drop(signal::ctrl_c().await);
    }
}

/// Start the HTTP server with optional TLS.
async fn start_server(
    app_state: AppState,
    listen_ip: IpAddr,
    listen_port: u16,
    tls_opt: Option<&TlsConfig>,
    config_path: &Path,
) -> eyre::Result<()> {
    let app = create_app(app_state);

    let addr = SocketAddr::from((listen_ip, listen_port));

    // Decide whether to serve plain HTTP or HTTPS depending on presence of config
    match tls_opt {
        Some(tls_cfg) => {
            let rustls_cfg = setup_tls_config(tls_cfg, config_path, listen_ip, addr).await?;
            let server = axum_server::bind_rustls(addr, rustls_cfg).serve(app);
            tokio::select! {
                res = server => res?,
                () = shutdown_signal() => {
                    info!("Received shutdown, shutting down");
                }
            }
        }
        _ => {
            info!("Listening on http://{}", addr);
            let listener = net::TcpListener::bind(addr).await?;
            let server = axum::serve(listener, app);
            tokio::select! {
                res = server => res?,
                () = shutdown_signal() => {
                    info!("Received shutdown, shutting down");
                }
            }
        }
    }

    Ok(())
}

/// # Returns
///
/// `Ok(())` when the server runs until termination, or an error if binding or setup fails.
///
/// # Errors
///
/// Returns an error if the configuration cannot be loaded, TLS setup fails, or the server cannot bind.
///
/// # Panics
///
/// Panics if the certificate path cannot be converted to a string.
pub(crate) async fn start(
    config_path: &Path,
    port_override: Option<u16>,
    bind_override: Option<&str>,
) -> eyre::Result<()> {
    info!("Starting HTTP server...");

    let (app_state, tls_opt) = initialize_state(config_path).await?;

    // Apply optional overrides from CLI/tests
    let listen_port = port_override.unwrap_or(app_state.config_rx.borrow().server.port);
    let bind_str = bind_override.map_or_else(
        || app_state.config_rx.borrow().server.bind.clone(),
        string::ToString::to_string,
    );

    let listen_ip = bind_str.parse()?;

    start_server(
        app_state,
        listen_ip,
        listen_port,
        tls_opt.as_ref(),
        config_path,
    )
    .await
}

/// Middleware to set security headers on all responses
///
/// This is less strict than possible.
/// it avoids using CORS, X-Frame-Options: DENY and corresponding CSP attributes,
/// since these might block some embedings etc.
async fn secure_headers_middleware(req: Request<Body>, next: Next) -> Response {
    let mut response = next.run(req).await;
    response.headers_mut().insert(
        HeaderName::from_static("cross-origin-opener-policy"),
        HeaderValue::from_static("same-origin"),
    );

    response.headers_mut().insert(
        HeaderName::from_static("content-security-policy"),
        HeaderValue::from_static(concat!(
            "default-src 'self'; ",
            "require-trusted-types-for 'script'; ",
            "script-src ",
            env!("CSP_INLINE_SCRIPTS_HASHES"),
            "; ",
            "manifest-src 'self'; ",
            // env!("CSP_MANIFEST_HASH"),
            // "'; ",
            "style-src-elem 'self'; ",
            // env!("CSP_STYLES_HASH"),
            // "'; ",
            "style-src-attr 'none'; ",
            "object-src 'none'; ",
            "base-uri 'none'; ",
            "frame-src 'none'; ",
            "media-src 'none'; ",
        )),
    );
    response.headers_mut().insert(
        HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );
    response
}
