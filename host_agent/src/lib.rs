//! Fake library entry for the `host_agent` crate.
//!
//! Houses the command-line interface for the `host_agent` binary, handling install, service launch, and `WoL` testing.

extern crate alloc;
extern crate core;

mod commands;
mod install;
pub mod registration;
pub mod script_generator;
pub mod server;
pub mod validation;

use std::env;

use clap::{Parser, Subcommand};

use server::ServiceOptions;

pub(crate) const VERSION: &str = shuthost_common::version_string!();

/// Build commit SHA embedded into the binary to allow staleness checking without executing it.
#[used]
static BUILD_COMMIT: &str = env!("SHUTHOST_BUILD_COMMIT");

use crate::install::BINARY_NAME;

/// Top-level CLI parser for `host_agent`.
#[derive(Debug, Parser)]
#[command(name = BINARY_NAME)]
#[command(version = VERSION)]
#[command(author = env!("CARGO_PKG_AUTHORS"))]
#[command(about = env!("CARGO_PKG_DESCRIPTION"))]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

/// Subcommands available for `host_agent` execution.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Start the `host_agent` as a background service.
    Service(ServiceOptions),

    /// Install the `host_agent` on the system.
    Install(install::Args),

    /// Update the already installed `host_agent` in place.
    ///
    /// If a self-extracting script is present in the current directory, it is preferred
    /// so the update can proceed without needing sudo for init-system service files.
    ///
    /// Use `--script-path` to point directly at a self-extracting script and skip autodetection.
    Update(install::UpdateArgs),

    /// Test Wake-on-LAN packet reachability on a given port.
    TestWol {
        /// UDP port to listen on for WOL test packets.
        #[arg(long, short, default_value_t = shuthost_common::DEFAULT_AGENT_TCP_PORT + 1)]
        port: u16,
    },

    /// Print the registration configuration for the installed agent.
    Registration(registration::Args),

    /// Generate a `shuthost_direct_control` script for this `host_agent`.
    #[clap(visible_alias = "gdc")]
    GenerateDirectControl(script_generator::Args),
}

pub fn inner_main(invocation: Cli) {
    match invocation.command {
        Command::Install(args) => match install::install_host_agent(&args) {
            Ok(()) => println!("Agent installed successfully!"),
            Err(e) => eprintln!("Error installing host_agent: {e}"),
        },
        Command::Update(args) => match install::update_host_agent(&args) {
            Ok(()) => println!("Agent updated successfully!"),
            Err(e) => eprintln!("Error updating host_agent: {e}"),
        },
        Command::Service(args) => {
            server::start_host_agent(args);
        }
        Command::TestWol { port } => match install::test_wol_reachability(port) {
            Ok(()) => (),
            Err(e) => eprintln!("Error during WoL test: {e}"),
        },
        Command::Registration(args) => match registration::parse_config(&args) {
            Ok(config) => {
                registration::print_registration_config(&config);
            }
            Err(e) => eprintln!("Error parsing config: {e}"),
        },
        Command::GenerateDirectControl(args) => {
            match script_generator::write_control_script(&args) {
                Ok(()) => (),
                Err(e) => eprintln!("Error generating direct control script: {e}"),
            }
        }
    }
}
