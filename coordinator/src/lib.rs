//! Fake library entry for the `coordinator` crate.
//!
//! Exposes `inner_main` so a workspace-level shim binary can call into the coordinator logic.
//!
//! Provides commands to install the service or launch the control web service.

pub mod auth;
pub mod cli;
pub mod config;
pub mod demo;
pub mod http;
pub mod install;
pub mod routes;
pub mod websocket;
pub mod wol;

// for use in integration tests
pub use websocket::WsMessage;

use std::fs;

use clap::Parser;
use eyre::{Result, WrapErr};
use tracing::info;
use tracing_subscriber::EnvFilter;

use cli::{Cli, Command};
use demo::run_demo_service;
use http::start;
use install::setup;

/// The coordinator's main function; can be called from a shim binary.
///
/// Parses CLI and dispatches install or server startup.
///
/// # Errors
///
/// Returns an error if installation fails or if the server fails to start.
///
/// # Panics
///
/// Panics if the AWS LC crypto provider cannot be installed.
pub async fn inner_main() -> Result<()> {
    let invocation = Cli::parse();

    match invocation.command {
        Command::Install(args) => {
            setup(args)?;
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

            // Pass through optional port/bind overrides from CLI
            start(&config_path, args.port, args.bind.as_deref()).await?;
            Ok(())
        }
        Command::DemoService { port, bind } => {
            run_demo_service(port, &bind).await;
            Ok(())
        }
    }
}
