//! Fake library entry for the `host_agent` crate.
//!
//! Houses the command-line interface for the `host_agent` binary, handling install, service launch, and WoL testing.

mod commands;
#[cfg(all(not(coverage), any(target_os = "linux", target_os = "macos")))]
mod install;
pub mod server;
pub mod validation;

use std::env;

use clap::{Parser, Subcommand};

#[cfg(all(not(coverage), any(target_os = "linux", target_os = "macos")))]
use install::{InstallArgs, install_host_agent};
use server::ServiceOptions;

/// Top-level CLI parser for host_agent.
#[derive(Debug, Parser)]
#[command(name = env!("CARGO_PKG_NAME"))]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(author = env!("CARGO_PKG_AUTHORS"))]
#[command(about = env!("CARGO_PKG_DESCRIPTION"))]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

/// Default UDP port on which the host_agent listens for commands.
pub const DEFAULT_PORT: u16 = 5757;

/// Subcommands available for host_agent execution.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Start the host_agent as a background service.
    Service(ServiceOptions),

    #[cfg(all(not(coverage), any(target_os = "linux", target_os = "macos")))]
    /// Install the host_agent on the system.
    Install(InstallArgs),

    #[cfg(not(coverage))]
    /// Test Wake-on-LAN packet reachability on a given port.
    TestWol {
        /// UDP port to listen on for WOL test packets.
        #[arg(long = "port", default_value_t = DEFAULT_PORT + 1)]
        port: u16,
    },
}

pub fn inner_main(invocation: Cli) {
    match invocation.command {
        #[cfg(all(not(coverage), any(target_os = "linux", target_os = "macos")))]
        Command::Install(args) => match install_host_agent(&args) {
            Ok(_) => println!("Agent installed successfully!"),
            Err(e) => eprintln!("Error installing host_agent: {e}"),
        },
        Command::Service(args) => {
            server::start_host_agent(args);
        }
        #[cfg(not(coverage))]
        Command::TestWol { port } => match install::test_wol_reachability(port) {
            Ok(_) => (),
            Err(e) => eprintln!("Error during WoL test: {e}"),
        },
    }
}
