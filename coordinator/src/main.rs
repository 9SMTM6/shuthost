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
}

/// Application entrypoint: parses CLI and dispatches install or server startup.
#[tokio::main]
async fn main() {
    let invocation = Cli::parse();

    match invocation.command {
        Command::Install(args) => {
            if let Err(e) = install_coordinator(args) {
                eprintln!("Error during installation: {}", e);
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
                eprintln!("Failed to start HTTP server: {}", e);
                std::process::exit(1);
            }
        }
    }
}
