//! Coordinator CLI entrypoint for the `shuthost_coordinator` binary.
//!
//! Provides commands to install the service or launch the control web service.

mod assets;
mod config;
mod http;
mod install;
mod routes;
mod websocket;
mod wol;

use axum::http::Response;
use clap::{Parser, Subcommand};
use install::{InstallArgs, install_coordinator};
use tracing_subscriber::EnvFilter;

use std::{env, fs};

use http::{ServiceArgs, start_http_server};
use tracing::info;

/// Top-level command-line interface definition.
#[derive(Debug, Parser)]
#[command(name = env!("CARGO_PKG_NAME"))]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = env!("CARGO_PKG_DESCRIPTION"))]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

/// Available subcommands for the coordinator.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Launch the control web service (WebUI) for managing hosts.
    ControlService(ServiceArgs),

    /// Install the coordinator service to start on boot.
    Install(InstallArgs),

    /// Serve only static assets for demo mode (no backend, no state).
    DemoService {
        #[arg(long, default_value = "8080")]
        port: u16,
        #[arg(long, default_value = "0.0.0.0")]
        bind: String,
    },
}

/// Application entrypoint: parses CLI and dispatches install or server startup.
#[tokio::main]
async fn main() {
    let invocation = Cli::parse();

    match invocation.command {
        Command::Install(args) => {
            if let Err(e) = install_coordinator(args) {
                eprintln!("Error during installation: {e}");
                std::process::exit(1);
            }
        }
        Command::ControlService(args) => {
            tracing_subscriber::fmt()
                .with_env_filter(
                    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
                )
                .pretty()
                .init(); // Initialize logging

            let config_path = match fs::canonicalize(&args.config) {
                Ok(path) => path,
                Err(_) => {
                    eprintln!("Config file not found at: {}", args.config);
                    std::process::exit(1);
                }
            };

            info!("Using config path: {}", config_path.display());

            if let Err(e) = start_http_server(&config_path).await {
                eprintln!("Failed to start HTTP server: {e}");
                std::process::exit(1);
            }
        }
        Command::DemoService { port, bind } => {
            run_demo_service(port, &bind).await;
        }
    }
}


// DemoService implementation: serves only static assets for demo mode
async fn run_demo_service(port: u16, bind: &str) {
    use axum::{Router};
    use tokio::net::TcpListener;
    use tracing::info;
    use crate::assets::asset_routes;
    use crate::routes::get_download_router;
    use crate::http::AppState;
    use std::sync::Arc;
    use tokio::sync::{broadcast, watch};
    use crate::config::ControllerConfig;
    use crate::routes::LeaseMap;
    use std::collections::HashMap;

    let addr = format!("{}:{}", bind, port);
    info!("Starting demo service on http://{}", addr);

    // Minimal dummy AppState for asset/download routes

    // Custom asset route for demo mode: inject disclaimer into HTML
    use axum::{extract::State, response::IntoResponse};
    use std::sync::OnceLock;
    async fn serve_demo_ui(State(_): State<AppState>) -> impl IntoResponse {
        use crate::assets::{render_ui_html, UiMode};
        static HTML_TEMPLATE: OnceLock<String> = OnceLock::new();
        let html = HTML_TEMPLATE.get_or_init(|| render_ui_html(UiMode::Demo)).clone();
        Response::builder()
            .header("Content-Type", "text/html")
            .body(html)
            .unwrap()
    }

    let app_state = AppState {
        config_path: std::path::PathBuf::from("demo"),
        config_rx: watch::channel(Arc::new(ControllerConfig::default())).1,
        hoststatus_rx: watch::channel(Arc::new(HashMap::new())).1,
        ws_tx: broadcast::channel(1).0,
        leases: LeaseMap::default(),
    };

        let app = Router::new()
            .route("/", axum::routing::get(serve_demo_ui))
            .merge(asset_routes())
            .nest("/download", get_download_router())
            .with_state(app_state);

    let listener = TcpListener::bind(&addr).await.expect("Failed to bind address");
    axum::serve(listener, app.into_make_service())
        .await
        .expect("Demo server failed");
}
