//! HTTP server implementation for the coordinator control interface.
//!
//! Defines routes, state management, configuration watching, and periodic host polling.

use axum::http::Request;
use axum::routing;
use axum::{Router, response::Redirect, routing::get};
use axum_server::tls_rustls::RustlsConfig as AxumRustlsConfig;
use std::{net::IpAddr, time::Duration};
use std::{net::SocketAddr, sync::Arc};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tracing::{debug, info, warn};

use crate::auth::{AuthRuntime, public_routes, require_auth};
use crate::{
    config::{ControllerConfig, load_coordinator_config, watch_config_file},
    routes::{LeaseMap, api_routes},
    websocket::{WsMessage, ws_handler},
};
use clap::Parser;
use shuthost_common::create_signed_message;
use std::collections::HashMap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
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

    /// Broadcast sender for distributing WebSocket messages.
    pub ws_tx: broadcast::Sender<WsMessage>,

    /// In-memory map of current leases for hosts.
    pub leases: LeaseMap,

    /// Authentication runtime (mode and secrets)
    pub auth: std::sync::Arc<AuthRuntime>,
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
pub async fn start_http_server(
    config_path: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting HTTP server...");

    let initial_config = Arc::new(load_coordinator_config(config_path).await?);
    let listen_port = initial_config.server.port;

    let listen_ip: IpAddr = initial_config.server.bind.parse()?;

    let (config_tx, config_rx) = watch::channel(initial_config.clone());

    let initial_status: Arc<HashMap<String, bool>> = Arc::new(HashMap::new());
    let (hoststatus_tx, hoststatus_rx) = watch::channel(initial_status);

    let (ws_tx, _) = broadcast::channel(32);

    {
        let config_rx = config_rx.clone();
        let hoststatus_tx = hoststatus_tx.clone();
        tokio::spawn(async move {
            poll_host_statuses(config_rx, hoststatus_tx).await;
        });
    }

    {
        let ws_tx = ws_tx.clone();
        let mut hoststatus_rx = hoststatus_rx.clone();
        tokio::spawn(async move {
            while hoststatus_rx.changed().await.is_ok() {
                let msg = WsMessage::HostStatus(hoststatus_rx.borrow().as_ref().clone());
                if ws_tx.send(msg).is_err() {
                    warn!("Failed to send WebSocket message");
                }
            }
        });
    }

    {
        let ws_tx = ws_tx.clone();
        let mut config_rx = config_rx.clone();
        tokio::spawn(async move {
            while config_rx.changed().await.is_ok() {
                let config = config_rx.borrow();
                let hosts = config.hosts.keys().cloned().collect::<Vec<_>>();
                let clients = config.clients.keys().cloned().collect::<Vec<_>>();
                let msg = WsMessage::ConfigChanged { hosts, clients };
                if ws_tx.send(msg).is_err() {
                    warn!("Failed to send WebSocket message");
                }
            }
        });
    }

    {
        let path = config_path.to_path_buf();
        let config_tx = config_tx.clone();
        tokio::spawn(async move {
            watch_config_file(path, config_tx).await;
        });
    }

    let auth_runtime = std::sync::Arc::new(AuthRuntime::from_config(&initial_config));

    let app_state = AppState {
        config_rx,
        hoststatus_rx,
        ws_tx,
        config_path: config_path.to_path_buf(),
        leases: LeaseMap::default(),
        auth: auth_runtime.clone(),
    };

    // Public routes (login, oidc callback, m2m endpoints, static assets such as PWA manifest, downloads for agent and client installs) must be reachable without auth
    let public = public_routes();

    // Private app routes protected by auth middleware
    let private = Router::new()
        .nest("/api", api_routes())
        .route("/", get(crate::assets::serve_ui))
        .route("/ws", get(ws_handler))
        .route_layer(axum::middleware::from_fn_with_state(
            crate::auth::AuthLayerState {
                auth: auth_runtime.clone(),
            },
            require_auth,
        ));

    let app = public
        .merge(private)
        .with_state(app_state)
        .fallback(routing::any(|req: Request<axum::body::Body>| async move {
            tracing::warn!(
                method = %req.method(),
                uri = %req.uri(),
                "Unhandled request"
            );
            Redirect::permanent("/")
        }));

    let addr = SocketAddr::from((listen_ip, listen_port));
    // Decide whether to serve plain HTTP or HTTPS depending on config
    let tls_cfg = &initial_config.server.tls;
    match tls_cfg.mode {
        // Provided certs: load PEM files and delegate to axum-server's rustls config
        crate::config::TlsMode::Provided => {
            let cert_path = tls_cfg
                .cert_path
                .as_ref()
                .ok_or("TLS mode 'Provided' requires cert_path")?;
            let key_path = tls_cfg
                .key_path
                .as_ref()
                .ok_or("TLS mode 'Provided' requires key_path")?;

            let rustls_cfg = AxumRustlsConfig::from_pem_file(cert_path, key_path).await?;
            info!("Listening on https://{} (provided certs)", addr);
            axum_server::bind_rustls(addr, rustls_cfg)
                .serve(app.into_make_service())
                .await?;
        }
        // Self-signed: generate a temporary cert/key pair and use them
        crate::config::TlsMode::SelfSigned => {
            // Use the listen host as CN/SAN when possible
            let hostnames = vec![listen_ip.to_string()];
            let rcgen::CertifiedKey { cert, signing_key } =
                rcgen::generate_simple_self_signed(hostnames)
                    .map_err(|e| format!("Failed to generate self-signed certificate: {}", e))?;
            // `cert.pem()` and `signing_key.serialize_pem()` give PEM-encoded strings
            let cert_pem = cert.pem();
            let key_pem = signing_key.serialize_pem();

            let rustls_cfg =
                AxumRustlsConfig::from_pem(cert_pem.into_bytes(), key_pem.into_bytes()).await?;
            info!("Listening on https://{} (self-signed)", addr);
            axum_server::bind_rustls(addr, rustls_cfg)
                .serve(app.into_make_service())
                .await?;
        }
        crate::config::TlsMode::Off => {
            info!("Listening on http://{}", addr);
            let listener = tokio::net::TcpListener::bind(addr).await?;
            axum::serve(listener, app.into_make_service()).await?;
        }
    }

    Ok(())
}

/// Background task: periodically polls each host for status by attempting a TCP connection and HMAC ping.
async fn poll_host_statuses(
    config_rx: watch::Receiver<Arc<ControllerConfig>>,
    hoststatus_tx: watch::Sender<Arc<HashMap<String, bool>>>,
) {
    loop {
        let config = config_rx.borrow().clone();

        let futures = config.hosts.iter().map(|(name, host)| {
            let addr = format!("{}:{}", host.ip, host.port);
            let name = name.clone();
            let shared_secret = host.shared_secret.clone();
            async move {
                let is_online =
                    match timeout(Duration::from_millis(500), TcpStream::connect(&addr)).await {
                        Ok(Ok(mut stream)) => {
                            let signed_message = create_signed_message("status", &shared_secret);
                            // Send message
                            if let Err(e) = stream.write_all(signed_message.as_bytes()).await {
                                debug!("Failed to write to {}: {}", name, e);
                                return (name, false);
                            }
                            // Read response (optional, but let's check for a valid reply)
                            let mut buf = vec![0u8; 256];
                            match timeout(Duration::from_millis(400), stream.read(&mut buf)).await {
                                Ok(Ok(n)) if n > 0 => {
                                    let Some(data) = buf.get(..n) else {
                                        unreachable!("Read data size should always be valid, as its >= buffer size");
                                    };
                                    let resp = String::from_utf8_lossy(data);
                                    // Accept any non-error response as online
                                    !resp.contains("ERROR")
                                }
                                _ => false,
                            }
                        }
                        _ => false,
                    };
                debug!("Polled {} at {} - online: {}", name, addr, is_online);
                (name, is_online)
            }
        });

        let results = futures::future::join_all(futures).await;
        let status_map: HashMap<_, _> = results.into_iter().collect();

        let is_new = {
            let old_status_map = hoststatus_tx.borrow();
            let old_status_map = old_status_map.as_ref();
            status_map != *old_status_map
        };
        if is_new {
            info!("Host status changed: {:?}", status_map);
            hoststatus_tx.send(Arc::new(status_map)).unwrap();
        } else {
            debug!("No change in host status");
        }

        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}
