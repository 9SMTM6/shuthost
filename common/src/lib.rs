//! Common utilities for HMAC handling and service management across supported platforms.
//!
//! This crate provides:
//! - Timestamped HMAC message signing and validation
//! - OS-specific service installation helpers

mod secrets;
#[cfg(not(coverage))]
mod service_install;
mod signing;
mod validation;

pub use secrets::*;
#[cfg(not(coverage))]
pub use service_install::*;
pub use signing::*;
pub use validation::*;

/// Returns `true` if the system uses systemd (detects `/run/systemd/system`).
pub fn is_systemd() -> bool {
    std::path::Path::new("/run/systemd/system").exists()
}
