//! Fake library entry for the `coordinator` crate.
//!
//! Exposes `inner_main` so a workspace-level shim binary can call into the coordinator logic.
//!
//! Provides commands to install the service or launch the control web service.

pub mod auth;
pub mod cli;
pub mod config;
pub mod db;
// Doesn't make sense to include in the coverage:
// Including a run of scripts/build-gh-pages.sh, which this is for,
// would artificially inflate coverage of endpoints that should be tested in actual use
#[cfg(not(coverage))]
pub mod demo;
pub mod http;
// Installation can't meaningfully be tested even in integration tests
// Its only exercised in CI
#[cfg(not(coverage))]
pub mod install;
pub mod routes;
pub mod websocket;
pub mod wol;

// for use in integration tests
pub use websocket::WsMessage;

use std::fs;
use std::sync::Once;

use eyre::{Result, WrapErr};
use tracing::info;
use tracing_subscriber::EnvFilter;

use cli::{Cli, Command};
#[cfg(not(coverage))]
use demo::run_demo_service;
use http::start;
#[cfg(not(coverage))]
use install::setup;

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
        #[cfg(not(coverage))]
        Command::Install(args) => {
            setup(args)?;
            Ok(())
        }
        Command::ControlService(args) => {
            let config_path = fs::canonicalize(&args.config)
                .wrap_err(format!("Config file not found at: {}", args.config))?;

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
                    .unwrap();
            });

            info!("Using config path: {}", config_path.display());

            // Pass through optional port/bind overrides from CLI
            start(&config_path, args.port, args.bind.as_deref()).await?;
            Ok(())
        }
        #[cfg(not(coverage))]
        Command::DemoService { port, bind } => {
            run_demo_service(port, &bind).await;
            Ok(())
        }
    }
}
