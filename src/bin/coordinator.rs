//! Shim binary that calls into the `coordinator` library's `inner_main`.
use eyre::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // Delegate to library entrypoint
    shuthost_coordinator::inner_main().await
}
