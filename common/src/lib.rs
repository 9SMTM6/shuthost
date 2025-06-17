//! Common utilities for HMAC handling and service management across supported platforms.
//!
//! This crate provides:
//! - Timestamped HMAC message signing and validation
//! - OS-specific service installation helpers

mod hmac;
mod service_install;

pub use hmac::*;
pub use service_install::*;
