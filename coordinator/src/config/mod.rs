//! Configuration management for the coordinator: loading and watching the TOML config file.
//!
//! This module provides a unified interface to all configuration-related functionality,
//! including data types, loading utilities, and file watching capabilities.

mod loader;
mod types;
mod watcher;

pub(crate) use loader::*;
pub(crate) use types::*;
pub(crate) use watcher::*;
