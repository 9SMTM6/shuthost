//! Common utilities for HMAC handling and service management across supported platforms.
//!
//! This crate provides:
//! - Timestamped HMAC message signing and validation
//! - OS-specific service installation helpers

mod secrets;
mod service_install;
mod signing;
mod validation;

pub use secrets::*;
pub use service_install::*;
pub use signing::*;
pub use validation::*;
