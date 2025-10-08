//! HTTP server implementation for the coordinator control interface.
//!
//! Defines routes, state management, configuration watching, and periodic host polling.

pub mod polling;
pub mod server;
pub mod assets;

pub use server::*;
