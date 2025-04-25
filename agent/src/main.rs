mod server;
mod handler;
mod install;

use std::env;
use std::path::PathBuf;
use install::install_agent;
use clap::{Parser, Subcommand};
use install::InstallArgs;
use server::ServiceArgs;

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

    /// Install the agent
    Install(InstallArgs),
}

fn main() {
    let binary_path = PathBuf::from(env::args().next().unwrap());
    
    let invocation = Cli::parse();

    match invocation.command {
        Command::Install(args) => {
            match install_agent(&binary_path, args) {
                Ok(_) => println!("Agent installed successfully!"),
                Err(e) => eprintln!("Error installing agent: {}", e),
            }
            return;
        }
        Command::Service(args) => {
            server::start_agent(args);
        }
    }
}
