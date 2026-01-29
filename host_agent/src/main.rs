//! Shim binary that calls into the `host_agent`s library's `inner_main`.

use clap::Parser as _;

fn main() {
    // Parse CLI arguments
    let invocation = shuthost_host_agent::Cli::parse();
    // Delegate to library entrypoint
    shuthost_host_agent::inner_main(invocation);
}
