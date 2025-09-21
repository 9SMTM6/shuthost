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
use eyre::{Result, WrapErr};
use http::start;
use install::install_coordinator;
use tracing::info;
use tracing_subscriber::EnvFilter;

/// Application entrypoint: parses CLI and dispatches install or server startup.
#[tokio::main]
async fn main() -> Result<()> {
    let invocation = Cli::parse();

    match invocation.command {
        Command::Install(args) => {
            install_coordinator(args)?;
            Ok(())
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

            let config_path = fs::canonicalize(&args.config)
                .wrap_err(format!("Config file not found at: {}", args.config))?;

            info!("Using config path: {}", config_path.display());

            start(&config_path).await?;
            Ok(())
        }
        Command::DemoService { port, bind } => {
            run_demo_service(port, &bind).await;
            Ok(())
        }
    }
}
