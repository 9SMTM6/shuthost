//! Shim binary that calls into the `coordinator` library's `inner_main`.
use eyre::Result;

use clap::Parser;
use shuthost_coordinator::cli::Cli;

#[tokio::main]
async fn main() -> Result<()> {
    let invocation = Cli::parse();
    // Delegate to library entrypoint
    shuthost_coordinator::inner_main(invocation).await
}
