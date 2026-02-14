//! Common utilities for HMAC handling and service management across supported platforms.
//!
//! This crate provides:
//! - Timestamped HMAC message signing and validation
//! - OS-specific service installation helpers
#![expect(
    clippy::missing_errors_doc,
    reason = "The situation it maps to errors should be obvious."
)]

extern crate alloc;
extern crate core;

use core::fmt;
use std::net::UdpSocket;
use std::path;

mod secrets;
mod service_install;
mod signing;
mod validation;

pub use secrets::*;
pub use service_install::*;
pub use signing::*;
pub use validation::*;

/// Extension traits for error handling to improve code coverage.
pub trait ResultMapErrExt<T> {
    fn map_err_to_string(self, prefix: &str) -> Result<T, String>;
    fn map_err_to_string_simple(self) -> Result<T, String>;
}

impl<T, E: fmt::Display> ResultMapErrExt<T> for Result<T, E> {
    fn map_err_to_string(self, prefix: &str) -> Result<T, String> {
        self.map_err(|e| format!("{prefix}: {e}"))
    }

    fn map_err_to_string_simple(self) -> Result<T, String> {
        self.map_err(|e| e.to_string())
    }
}

pub trait UnwrapToStringExt {
    fn unwrap_or_to_string(self, default: &str) -> String;
}

impl<T: ToString> UnwrapToStringExt for Option<T> {
    fn unwrap_or_to_string(self, default: &str) -> String {
        self.map(|t| t.to_string()).unwrap_or(default.to_string())
    }
}

impl<T: ToString, E> UnwrapToStringExt for Result<T, E> {
    fn unwrap_or_to_string(self, default: &str) -> String {
        self.map(|t| t.to_string()).unwrap_or(default.to_string())
    }
}

/// Creates a UDP socket configured for broadcasting on the specified port.
///
/// Binds to the given port on all interfaces and enables broadcasting.
/// If port is 0, binds to any available port.
/// Returns the socket if successful, or an error message if binding or setting broadcast fails.
pub fn create_broadcast_socket(port: u16) -> Result<UdpSocket, String> {
    let addr = if port == 0 {
        "0.0.0.0:0".to_string()
    } else {
        format!("0.0.0.0:{port}")
    };
    let socket = UdpSocket::bind(&addr)
        .map_err(|e| format!("Failed to bind socket on {addr}: {e}"))?;
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
