//! Fake library entry for the `coordinator` crate.
//!
//! Exposes `inner_main` so a workspace-level shim binary can call into the coordinator logic.
//!
//! Provides commands to install the service or launch the control web service.
#![cfg_attr(
    test,
    expect(clippy::indexing_slicing, reason = "This is not problematic in tests",)
)]

extern crate alloc;
extern crate core;

pub mod cli;
pub mod config;
pub mod db;
pub mod demo;
pub mod http;
#[cfg(unix)]
pub mod install;
pub mod run;
pub mod state;
pub mod websocket;
pub mod wol;

#[cfg(unix)]
use nix::sys::stat;
use tracing::Instrument;
// for use in integration tests
pub use websocket::WsMessage;

use std::env;
use std::fs;
use std::sync::Once;

use eyre::{Result, WrapErr as _};
use tracing::{info, warn};
use tracing_subscriber::{EnvFilter, fmt::time::ChronoLocal};

use cli::{Cli, Command, LogFormat};
use demo::run_demo_service;

use crate::run::start;

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
/// Panics if the OpenSSL crypto provider cannot be installed.
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

            INIT_TRACING.call_once(move || {
                let default_level = if env::var("SHUTHOST_INTEGRATION_TEST").is_ok() {
                    "error"
                } else {
                    "info"
                };

                let builder = tracing_subscriber::fmt()
                    .with_env_filter(
                        EnvFilter::try_from_default_env()
                            .unwrap_or_else(|_| EnvFilter::new(default_level)),
                    )
                    .with_timer(ChronoLocal::rfc_3339());

                match args.log_format {
                    LogFormat::Compact => builder.compact().init(),
                    LogFormat::Json => builder.json().init(),
                    LogFormat::Pretty => builder.pretty().init(),
                }
            });

            // Create a startup span that holds the resolved config path for the lifetime
            // of the coordinator initialization phase.
            let startup_span = tracing::info_span!("coord.startup", ?config_path, pid=?std::process::id(), version = env!("CARGO_PKG_VERSION"));
            let _startup_enter = startup_span.enter();

            INIT_RUSTLS.call_once(|| {
                rustls_openssl::default_provider()
                    .install_default()
                    .expect("failed to install default rustls provider");
            });

            for warning in env!("BUILD_WARNINGS").split(';') {
                warn!(warning);
            }

            info!("Starting coordinator");

            // Pass through optional port/bind overrides from CLI
            start(&config_path, args.port, args.bind.as_deref())
                .in_current_span()
                .await?;
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
