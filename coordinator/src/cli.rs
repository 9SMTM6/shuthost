//! Command-line interface definitions for the coordinator.
//!
//! This module contains the CLI argument parsing structures and enums
//! used by the main coordinator binary.

use std::env;

use crate::install;
use clap::{Parser, Subcommand};

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
    /// Launch the control web service (`WebUI`) for managing hosts.
    ControlService(ServiceArgs),

    #[cfg(unix)]
    /// Install the coordinator service to start on boot.
    Install(install::Args),

    /// Serve only static assets for demo mode (no backend, no state).
    DemoService {
        #[arg(long, default_value = "8080")]
        port: u16,
        #[arg(long, default_value = "0.0.0.0")]
        bind: String,
        /// Subpath where the demo is hosted (e.g. "/shuthost").
        /// Defaults to `/` and is a positional argument.
        #[arg(default_value = "/")]
        subpath: String,
    },
}

/// Arguments for the control service command.
#[derive(Debug, Parser)]
pub struct ServiceArgs {
    /// Path to the configuration file
    #[arg(
        short,
        long,
        env = "SHUTHOST_CONTROLLER_CONFIG_PATH",
        default_value = "shuthost_coordinator.toml"
    )]
    pub config: String,
    /// Optional override for the listen port (overrides port in config)
    #[arg(long)]
    pub port: Option<u16>,

    /// Optional override for the bind address (overrides bind in config)
    #[arg(long)]
    pub bind: Option<String>,
}
