//! Shim binary that calls into the `host_agent`s library's `inner_main`.

fn main() {
    // Delegate to library entrypoint
    shuthost_host_agent::inner_main();
}
