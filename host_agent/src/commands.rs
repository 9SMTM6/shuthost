//! Command execution utilities for the host agent.
//!
//! This module provides functions for executing system commands,
//! particularly shutdown commands received from the coordinator.

use crate::server::ServiceOptions;

/// Executes the configured shutdown command via the appropriate shell for the platform.
///
/// # Arguments
///
/// * `config` - `ServiceArgs` holding the `shutdown_command` to execute.
///
/// # Errors
///
/// Returns `Err` if spawning or waiting on the process fails.
pub(crate) fn execute_shutdown(config: &ServiceOptions) -> Result<(), std::io::Error> {
    println!("Executing command: {}", &config.shutdown_command);

    const IS_WINDOWS: bool = cfg!(target_os = "windows");

    std::process::Command::new(if IS_WINDOWS { "pwsh" } else { "sh" })
        .arg(if IS_WINDOWS { "-Command" } else { "-c" })
        .arg(&config.shutdown_command)
        .spawn()?
        .wait()?;

    Ok(())
}
