//! Command execution utilities for the host agent.
//!
//! This module provides functions for executing system commands,
//! particularly shutdown commands received from the coordinator.

use crate::server::ServiceArgs;

/// Executes the configured shutdown command via the shell.
///
/// # Arguments
///
/// * `config` - ServiceArgs holding the `shutdown_command` to execute.
///
/// # Errors
///
/// Returns `Err` if spawning or waiting on the process fails.
pub fn execute_shutdown(config: &ServiceArgs) -> Result<(), std::io::Error> {
    println!("Executing command: {}", &config.shutdown_command);
    std::process::Command::new("sh")
        .arg("-c")
        .arg(&config.shutdown_command)
        .spawn()?
        .wait()?;
    Ok(())
}
