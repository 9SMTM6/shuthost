//! Common utilities for HMAC handling and service management across supported platforms.
//!
//! This crate provides:
//! - Timestamped HMAC message signing and validation
//! - OS-specific service installation helpers

extern crate alloc;
extern crate core;

mod map_to_str;
pub mod protocol;
mod service_install;
mod signing;
mod validation;

use std::{net::UdpSocket, path};

pub use map_to_str::*;
pub use protocol::*;
pub use service_install::*;
pub use signing::*;
pub use validation::*;

/// Default UDP port that agents use to announce themselves via broadcast.
///
/// The coordinator listens on this port for incoming broadcasts.  If this value
/// is changed the shell installers (see `scripts/coordinator_installers/…`)
/// must be updated as they hard‑code the constant when invoking the agent.
pub const DEFAULT_COORDINATOR_BROADCAST_PORT: u16 = 5757;

/// Default TCP port on which an agent listens for control commands.
///
/// This is used both as the default for CLI parsing inside `host_agent` and in
/// the various installer templates.  Again, installers must be manually updated
/// if this value changes.
pub const DEFAULT_AGENT_TCP_PORT: u16 = 9090;

/// Creates a UDP socket configured for broadcasting on the specified port.
///
/// Binds to the given port on all interfaces and enables broadcasting.
/// If port is 0, binds to any available port.
/// Returns the socket if successful, or an error message if binding or setting broadcast fails.
///
/// # Errors
/// Returns `Err` if the socket cannot be bound or broadcast cannot be enabled.
pub fn create_broadcast_socket(port: u16) -> Result<UdpSocket, String> {
    let addr = format!("0.0.0.0:{port}");
    let socket =
        UdpSocket::bind(&addr).map_err(|e| format!("Failed to bind socket on {addr}: {e}"))?;
    socket
        .set_broadcast(true)
        .map_err(|e| format!("Failed to set broadcast on socket: {e}"))?;
    Ok(socket)
}

/// Returns `true` if the system uses systemd (detects `/run/systemd/system`).
#[must_use]
pub fn is_systemd() -> bool {
    path::Path::new("/run/systemd/system").exists()
}
