#![expect(
    clippy::missing_errors_doc,
    reason = "The situations these helper functions map to errors should be obvious."
)]

//! This module provides ext functions that avoid bloat of mapper functions at every call site that (IMO) artificially suppress function coverage.
use core::fmt;

/// Extension trait for error handling.
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

/// Extension trait for converting `Option` and `Result` types to `String` with a default value.
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
