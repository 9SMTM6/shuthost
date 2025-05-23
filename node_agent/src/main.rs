mod handler;
mod install;
mod server;

use clap::{Parser, Subcommand};
use install::InstallArgs;
use install::install_node_agent;
use install::DEFAULT_PORT;
use server::ServiceArgs;
use std::env;

#[derive(Debug, Parser)]
#[command(name = env!("CARGO_PKG_NAME"))]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(author = env!("CARGO_PKG_AUTHORS"))]
#[command(about = env!("CARGO_PKG_DESCRIPTION"))]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Start the service
    Service(ServiceArgs),

    /// Install the node_agent
    Install(InstallArgs),

    /// Test WoL packet reachability
    TestWol {
        #[arg(long = "port", default_value_t = DEFAULT_PORT + 1)]
        port: u16,
    },
}

fn main() {
    let invocation = Cli::parse();

    match invocation.command {
        Command::Install(args) => match install_node_agent(args) {
            Ok(_) => println!("Agent installed successfully!"),
            Err(e) => eprintln!("Error installing node_agent: {}", e),
        },
        Command::Service(args) => {
            server::start_node_agent(args);
        },
        Command::TestWol { port } => match install::test_wol_reachability(port) {
            Ok(_) => (),
            Err(e) => eprintln!("Error during WoL test: {}", e),
        },
    }
}
