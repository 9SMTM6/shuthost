mod config;
mod http;
mod install;
mod routes;
mod wol;

use clap::{Parser, Subcommand};
use install::{InstallArgs, install_coordinator};

use std::{env, fs};

use http::{ServiceArgs, start_http_server};
use tracing::info;

#[derive(Debug, Parser)]
#[command(name = env!("CARGO_PKG_NAME"))]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = env!("CARGO_PKG_DESCRIPTION"))]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Start the WebUI
    ControlService(ServiceArgs),

    /// Install the WebUI as system service, starting on boot
    Install(InstallArgs),
}

#[tokio::main]
async fn main() {
    let invocation = Cli::parse();

    // TODO: dont panic, throw proper errors and exit.

    match invocation.command {
        Command::Install(args) => {
            install_coordinator(args).unwrap();
        }
        Command::ControlService(args) => {
            tracing_subscriber::fmt()
                .with_env_filter(
                    tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_default(),
                )
                .pretty()
                .init(); // Initialize logging
            let config_path = fs::canonicalize(&args.config)
                .unwrap_or_else(|_| panic!("Config file not found at: {}", args.config));
            info!("Using config path: {}", config_path.display());
            start_http_server(&config_path).await;
        }
    }
}
