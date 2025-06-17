//! Command-line interface for the `host_agent` binary, handling install, service launch, and WoL testing.

mod handler;
mod install;
mod server;

use clap::{Parser, Subcommand};
use install::DEFAULT_PORT;
use install::InstallArgs;
use install::install_host_agent;
use server::ServiceArgs;
use std::env;

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

/// Subcommands available for host_agent execution.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Start the host_agent as a background service.
    Service(ServiceArgs),

    /// Install the host_agent on the system.
    Install(InstallArgs),

    /// Test Wake-on-LAN packet reachability on a given port.
    TestWol {
        /// UDP port to listen on for WOL test packets.
        #[arg(long = "port", default_value_t = DEFAULT_PORT + 1)]
        port: u16,
    },
}

fn main() {
    let invocation = Cli::parse();

    match invocation.command {
        Command::Install(args) => match install_host_agent(args) {
            Ok(_) => println!("Agent installed successfully!"),
            Err(e) => eprintln!("Error installing host_agent: {}", e),
        },
        Command::Service(args) => {
            server::start_host_agent(args);
        }
        Command::TestWol { port } => match install::test_wol_reachability(port) {
            Ok(_) => (),
            Err(e) => eprintln!("Error during WoL test: {}", e),
        },
    }
}
