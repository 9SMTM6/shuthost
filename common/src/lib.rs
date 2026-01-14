//! Common utilities for HMAC handling and service management across supported platforms.
//!
//! This crate provides:
//! - Timestamped HMAC message signing and validation
//! - OS-specific service installation helpers
#![expect(
    clippy::missing_errors_doc,
    reason = "The situation it maps to errors should be obvious."
)]

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

impl<T, E: std::fmt::Display> ResultMapErrExt<T> for Result<T, E> {
    fn map_err_to_string(self, prefix: &str) -> Result<T, String> {
        self.map_err(|e| format!("{}: {}", prefix, e))
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

/// Returns `true` if the system uses systemd (detects `/run/systemd/system`).
pub fn is_systemd() -> bool {
    std::path::Path::new("/run/systemd/system").exists()
}
