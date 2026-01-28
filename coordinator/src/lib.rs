//! Fake library entry for the `coordinator` crate.
//!
//! Exposes `inner_main` so a workspace-level shim binary can call into the coordinator logic.
//!
//! Provides commands to install the service or launch the control web service.

pub mod auth;
pub mod cli;
pub mod config;
pub mod db;
pub mod demo;
pub mod http;
#[cfg(unix)]
pub mod install;
pub mod websocket;
pub mod wol;

#[cfg(unix)]
use nix::sys::stat;
// for use in integration tests
pub use websocket::WsMessage;

use std::fs;
use std::sync::Once;

use eyre::{Result, WrapErr};
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

use cli::{Cli, Command};
use demo::run_demo_service;
use http::start;

static INIT_TRACING: Once = Once::new();
static INIT_RUSTLS: Once = Once::new();

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
pub async fn inner_main(invocation: Cli) -> Result<()> {
    match invocation.command {
        #[cfg(unix)]
        Command::Install(args) => {
            install::setup(args)?;
            Ok(())
        }
        Command::ControlService(args) => {
            // Set umask to ensure database files have restrictive permissions
            #[cfg(unix)]
            stat::umask(stat::Mode::S_IRWXU.complement());

            let config = &args.config;
            let config_path =
                fs::canonicalize(config).wrap_err(format!("Config file not found at: {config}"))?;

            INIT_TRACING.call_once(|| {
                let default_level = if std::env::var("SHUTHOST_INTEGRATION_TEST").is_ok() {
                    "error"
                } else {
                    "info"
                };
                tracing_subscriber::fmt()
                    .with_env_filter(
                        EnvFilter::try_from_default_env()
                            .unwrap_or_else(|_| EnvFilter::new(default_level)),
                    )
                    .pretty()
                    .init(); // Initialize logging
            });

            INIT_RUSTLS.call_once(|| {
                rustls::crypto::aws_lc_rs::default_provider()
                    .install_default()
                    .expect("failed to install default rustls provider");
            });

            for warning in env!("BUILD_WARNINGS").split(";") {
                warn!(warning);
            }

            info!("Using config path: {}", config_path.display());

            // Pass through optional port/bind overrides from CLI
            start(&config_path, args.port, args.bind.as_deref()).await?;
            Ok(())
        }
        Command::DemoService {
            port,
            bind,
            subpath,
        } => {
            run_demo_service(port, &bind, &subpath).await;
            Ok(())
        }
    }
}
