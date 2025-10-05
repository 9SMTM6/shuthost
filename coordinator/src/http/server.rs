//! HTTP server implementation for the coordinator control interface.
//!
//! Defines routes, state management, configuration watching, and server startup.

use axum::body::Body;
use axum::http::Request;
use axum::http::header::{AUTHORIZATION, COOKIE};
use axum::routing::{self, any};
use axum::{Router, response::Redirect, routing::get};
use axum_server::tls_rustls::RustlsConfig as AxumRustlsConfig;
use eyre::WrapErr;
use std::path::Path;
use std::time::Duration;
use std::{net::IpAddr, sync::Arc};
use tokio::fs;
use tower::ServiceBuilder;
use tower_http::ServiceBuilderExt as _;
use tower_http::request_id::MakeRequestUuid;
use tower_http::timeout::TimeoutLayer;
use axum::http::HeaderValue;
use axum::middleware::Next;
use axum::response::Response;
use axum::http::header::{HeaderName};
use tracing::{info, warn};

use crate::auth::{AuthRuntime, public_routes, require_auth};
use crate::config::TlsConfig;
use crate::{
    config::{ControllerConfig, load_coordinator_config},
    routes::{LeaseMap, api_routes},
    websocket::{WsMessage, ws_handler},
};
use clap::Parser;
use std::collections::HashMap;
use tokio::sync::{broadcast, watch};

/// Command-line arguments for the HTTP service subcommand.
#[derive(Debug, Parser)]
pub struct ServiceArgs {
    /// Path to the coordinator TOML config file.
    #[arg(
        long = "config",
        env = "SHUTHOST_CONTROLLER_CONFIG_PATH",
        default_value = "shuthost_coordinator.toml"
    )]
    pub config: String,
}

/// Application state shared across request handlers and background tasks.
#[derive(Clone)]
pub struct AppState {
    /// Path to the configuration file for template injection and reloads.
    pub config_path: std::path::PathBuf,

    /// Receiver for updated `ControllerConfig` when the file changes.
    pub config_rx: watch::Receiver<Arc<ControllerConfig>>,

    /// Receiver for host online/offline status updates.
    pub hoststatus_rx: watch::Receiver<Arc<HashMap<String, bool>>>,
    /// Sender for host online/offline status updates.
    pub hoststatus_tx: watch::Sender<Arc<HashMap<String, bool>>>,

    /// Broadcast sender for distributing WebSocket messages.
    pub ws_tx: broadcast::Sender<WsMessage>,

    /// In-memory map of current leases for hosts.
    pub leases: LeaseMap,

    /// Authentication runtime (mode and secrets)
    pub auth: std::sync::Arc<AuthRuntime>,
    /// Whether the HTTP server was started with TLS enabled (true for HTTPS)
    pub tls_enabled: bool,
}

/// Starts the Axum-based HTTP server for the coordinator UI and API.
///
/// # Arguments
///
/// * `config_path` - Path to the TOML configuration file.
///
/// # Returns
///
/// `Ok(())` when the server runs until termination, or an error if binding or setup fails.
pub async fn start(config_path: &std::path::Path) -> eyre::Result<()> {
    info!("Starting HTTP server...");

    let initial_config = Arc::new(load_coordinator_config(config_path).await?);
    let listen_port = initial_config.server.port;

    let listen_ip: IpAddr = initial_config.server.bind.parse()?;

    let (config_tx, config_rx) = watch::channel(initial_config.clone());

    let initial_status: Arc<HashMap<String, bool>> = Arc::new(HashMap::new());
    let (hoststatus_tx, hoststatus_rx) = watch::channel(initial_status);

    let (ws_tx, _) = broadcast::channel(32);

    // Start background tasks
    crate::http::polling::start_background_tasks(
        &config_rx,
        &hoststatus_tx,
        &ws_tx,
        &config_tx,
        config_path,
    );

    let auth_runtime = std::sync::Arc::new(AuthRuntime::from_config(&initial_config));

    // Startup-time warning: if TLS is not enabled but authentication is active,
    // browsers will ignore cookies marked Secure. Warn operators so they can
    // enable TLS or place the app behind an HTTPS reverse proxy that sets
    // X-Forwarded-Proto: https.
    let tls_opt = match initial_config.server.tls {
        Some(ref tls_cfg @ TlsConfig { enable: true, .. }) => Some(tls_cfg),
        _ => None,
    };
    if tls_opt.is_none() {
        match &auth_runtime.mode {
            &crate::auth::AuthResolved::Disabled => {}
            _ => {
                warn!(
                    "TLS appears disabled but authentication is enabled. Authentication cookies are set with Secure=true and will not be sent by browsers over plain HTTP. Enable TLS or run behind an HTTPS reverse proxy (ensure it sets X-Forwarded-Proto: https)."
                );
            }
        }
    }

    let app_state = AppState {
        config_rx,
        hoststatus_rx,
        hoststatus_tx,
        ws_tx,
        config_path: config_path.to_path_buf(),
        leases: LeaseMap::default(),
        auth: auth_runtime.clone(),
        tls_enabled: tls_opt.is_some(),
    };

    // Public routes (login, oidc callback, m2m endpoints, static assets such as PWA manifest, downloads for agent and client installs) must be reachable without auth
    let public = public_routes();

    // Private app routes protected by auth middleware
    let private = Router::new()
        .nest("/api", api_routes())
        .route("/", get(crate::assets::serve_ui))
        .route("/ws", any(ws_handler))
        .route_layer(axum::middleware::from_fn_with_state(
            crate::auth::AuthLayerState {
                auth: auth_runtime.clone(),
            },
            require_auth,
        ));

    // TODO: figure out rate limiting
    let middleware_stack = ServiceBuilder::new()
        .sensitive_headers([AUTHORIZATION, COOKIE])
        .set_x_request_id(MakeRequestUuid)
        .propagate_x_request_id()
        // must be after request-id
        .trace_for_http()
        .compression()
        .layer(TimeoutLayer::new(Duration::from_secs(30)))
        .layer(axum::middleware::from_fn(secure_headers_middleware));

    let app = public
        .merge(private)
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

    let addr = std::net::SocketAddr::from((listen_ip, listen_port));
    // Decide whether to serve plain HTTP or HTTPS depending on presence of config
    match tls_opt {
        Some(tls_cfg) => {
            // Helper: resolve a configured path relative to the config file unless it's absolute
            let resolve_path = |p: &str| {
                let path = std::path::Path::new(p);
                if path.is_absolute() {
                    path.to_path_buf()
                } else {
                    config_path
                        .parent()
                        .map(|d| d.join(path))
                        .unwrap_or_else(|| std::path::PathBuf::from(path))
                }
            };

            // Use provided certs when both files exist. Otherwise, if persist_self_signed is true
            // (default), generate and persist self-signed cert/key next to the config file.
            let cert_path_cfg = tls_cfg.cert_path.as_str();
            let key_path_cfg = tls_cfg.key_path.as_str();

            let cert_path = resolve_path(cert_path_cfg);
            let key_path = resolve_path(key_path_cfg);

            let cert_exists = cert_path.exists();
            let key_exists = key_path.exists();

            let rustls_cfg = if cert_exists && key_exists {
                let rustls_cfg = AxumRustlsConfig::from_pem_file(
                    cert_path.to_str().unwrap(),
                    key_path.to_str().unwrap(),
                )
                .await?;
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
                let key_pem = signing_key.serialize_pem();

                // Ensure parent dir exists (typically same dir as config)
                let cfg_dir = config_path.parent().unwrap_or_else(|| Path::new("."));
                fs::create_dir_all(cfg_dir).await?;

                // Write cert/key files
                tokio::try_join!(
                    fs::write(&cert_path, cert_pem.as_bytes()),
                    fs::write(&key_path, key_pem.as_bytes())
                )?;

                let rustls_cfg =
                    AxumRustlsConfig::from_pem(cert_pem.into_bytes(), key_pem.into_bytes()).await?;
                info!(
                    "Listening on https://{} (self-signed, persisted at {:?})",
                    addr, cfg_dir
                );
                rustls_cfg
            } else {
                eyre::bail!(
                    "TLS configuration error: neither provided certs nor self-signed allowed"
                );
            };
            axum_server::bind_rustls(addr, rustls_cfg)
                .serve(app.into_make_service())
                .await?;
        }
        _ => {
            info!("Listening on http://{}", addr);
            let listener = tokio::net::TcpListener::bind(addr).await?;
            axum::serve(listener, app.into_make_service()).await?;
        }
    };

    Ok(())
}

/// Middleware to set security headers on all responses
/// 
/// This is less strict than possible. 
/// it avoids using CORS, X-Frame-Options: DENY and corresponding CSP attributes,
/// since these might block some embedings etc.
/// 
/// It also allows inlined scripts (!) and doesn't require-trusted-types-for, 
/// since these have limited compatibility and/or require a bundler for effective application management
/// 
/// These would help against clickjacking etc.
async fn secure_headers_middleware(req: Request<axum::body::Body>, next: Next) -> Response {
    let mut response = next.run(req).await;
    response.headers_mut().insert(
        HeaderName::from_static("cross-origin-opener-policy"),
        HeaderValue::from_static("same-origin"),
    );
    response.headers_mut().insert(
        HeaderName::from_static("content-security-policy"),
        HeaderValue::from_static(
            "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self'; object-src 'none'; base-uri 'self';"
        ),
    );
    response
}
