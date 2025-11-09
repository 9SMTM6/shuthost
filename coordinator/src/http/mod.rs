//! HTTP server implementation for the coordinator control interface.
//!
//! Defines routes, state management, configuration watching, and periodic host polling.

pub mod api;
pub mod assets;
pub mod download;
pub mod login;
pub mod m2m;
pub mod polling;
pub mod server;

pub use server::*;
