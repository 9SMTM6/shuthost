use alloc::string;
use core::net::{IpAddr, SocketAddr};
use std::path::Path;

use tokio::{net, signal};

use crate::{
    config::TlsConfig,
    http::{router, tls::setup_tls_config},
    state::{self, AppState},
};

/// Creates a future that resolves when a shutdown signal is received.
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
pub(crate) async fn start_server(
    app_state: AppState,
    listen_ip: IpAddr,
    listen_port: u16,
    tls_opt: Option<&TlsConfig>,
    config_path: &Path,
) -> eyre::Result<()> {
    let app = router::create_app(app_state);

    let addr = SocketAddr::from((listen_ip, listen_port));

    match tls_opt {
        Some(tls_cfg) => {
            let rustls_cfg = setup_tls_config(tls_cfg, config_path, listen_ip, addr).await?;
            let server = axum_server::bind_rustls(addr, rustls_cfg).serve(app);
            tokio::select! {
                res = server => res?,
                () = shutdown_signal() => {
                    tracing::info!("Received shutdown, shutting down");
                }
            }
        }
        _ => {
            tracing::info!("Listening on http://{}", addr);
            let listener = net::TcpListener::bind(addr).await?;
            let server = axum::serve(listener, app);
            tokio::select! {
                res = server => res?,
                () = shutdown_signal() => {
                    tracing::info!("Received shutdown, shutting down");
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
    tracing::info!("Starting HTTP server...");

    let (app_state, tls_opt) = state::initialize_state(config_path).await?;

    // Apply optional overrides from CLI/tests
    let listen_port = port_override.unwrap_or(app_state.config_rx.borrow().server.port);
    let bind_str = bind_override.map_or_else(
        || app_state.config_rx.borrow().server.bind.clone(),
        string::ToString::to_string,
    );

    let listen_ip: IpAddr = bind_str.parse()?;

    start_server(
        app_state,
        listen_ip,
        listen_port,
        tls_opt.as_ref(),
        config_path,
    )
    .await
}
