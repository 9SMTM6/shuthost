//! Fake library entry for the `host_agent` crate.
//!
//! Houses the command-line interface for the `host_agent` binary, handling install, service launch, and `WoL` testing.

mod commands;
mod install;
pub mod registration;
pub mod script_generator;
pub mod server;
pub mod validation;

use std::env;

use clap::{Parser, Subcommand};

use server::ServiceOptions;

/// Top-level CLI parser for `host_agent`.
#[derive(Debug, Parser)]
#[command(name = env!("CARGO_PKG_NAME"))]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(author = env!("CARGO_PKG_AUTHORS"))]
#[command(about = env!("CARGO_PKG_DESCRIPTION"))]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

/// Default UDP port on which the `host_agent` listens for commands.
pub const DEFAULT_PORT: u16 = 5757;

/// Subcommands available for `host_agent` execution.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Start the `host_agent` as a background service.
    Service(ServiceOptions),

    /// Install the `host_agent` on the system.
    Install(install::Args),

    /// Test Wake-on-LAN packet reachability on a given port.
    TestWol {
        /// UDP port to listen on for WOL test packets.
        #[arg(long, short, default_value_t = DEFAULT_PORT + 1)]
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
