//! Request handler utilities for host_agent: parsing, validating, and executing shutdown commands.

use crate::server::ServiceArgs;
use shuthost_common::validate_hmac_message;

/// Parses incoming bytes, validates HMAC-signed commands, and returns a response with a flag indicating shutdown.
///
/// # Arguments
///
/// * `data` - Raw request bytes received over TCP.
/// * `config` - Shared service configuration including the secret and shutdown command.
/// * `peer_addr` - String representation of the client's address for logging.
///
/// # Returns
///
/// A tuple `(response, should_shutdown)`, where `response` is sent back to the client, and
/// `should_shutdown` indicates if the agent should execute a shutdown.
pub fn handle_request_without_shutdown(
    data: &[u8],
    config: &ServiceArgs,
    peer_addr: &str,
) -> (String, bool) {
    let data_str = match std::str::from_utf8(data) {
        Ok(s) => s,
        Err(_) => {
            eprintln!("Invalid UTF-8 in request from {}: {:?}", peer_addr, data);
            return ("ERROR: Invalid UTF-8".to_string(), false);
        }
    };

    match validate_hmac_message(data_str, &config.shared_secret) {
        shuthost_common::HmacValidationResult::Valid(command) => {
            // Proceed with valid command

            // Special handling for status check command
            if command == "status" {
                return ("OK: status".to_string(), false);
            }

            // Only allow the expected shutdown command
            if command != "shutdown" {
                eprintln!("Invalid command from {}: {}", peer_addr, command);
                return ("ERROR: Invalid command".to_string(), false);
            }

            (
                format!(
                    "Now executing command: {}. Hopefully goodbye.",
                    config.shutdown_command
                ),
                true,
            )
        }
        shuthost_common::HmacValidationResult::InvalidTimestamp => {
            eprintln!("Timestamp out of range from {}", peer_addr);
            return ("ERROR: Timestamp out of range".to_string(), false);
        }
        shuthost_common::HmacValidationResult::InvalidHmac => {
            eprintln!("Invalid HMAC signature from {}", peer_addr);
            return ("ERROR: Invalid HMAC signature".to_string(), false);
        }
        shuthost_common::HmacValidationResult::MalformedMessage => {
            eprintln!("Invalid request format from {}", peer_addr);
            return ("ERROR: Invalid request format".to_string(), false);
        }
    }
}

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
