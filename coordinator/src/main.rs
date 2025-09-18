//! Coordinator CLI entrypoint for the `shuthost_coordinator` binary.
//!
//! Provides commands to install the service or launch the control web service.

mod assets;
mod auth;
mod cli;
mod config;
mod demo;
mod http;
mod install;
mod routes;
mod websocket;
mod wol;

use std::fs;

use clap::Parser;
use cli::{Cli, Command};
use demo::run_demo_service;
use http::start_http_server;
use install::install_coordinator;
use tracing::info;
use tracing_subscriber::EnvFilter;

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

            rustls::crypto::aws_lc_rs::default_provider()
                .install_default()
                .unwrap();

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
