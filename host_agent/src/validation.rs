//! Request validation utilities for the host agent.
//!
//! This module provides functions for parsing and validating
//! incoming HMAC-signed requests from the coordinator.

use crate::server::ServiceOptions;
use shuthost_common::validate_hmac_message;

/// Possible actions the agent can take after validating a request.
#[derive(Debug, PartialEq)]
pub enum Action {
    /// No special action; just respond.
    None,
    /// Shut down the host machine.
    Shutdown,
    /// Abort (stop) the host agent service.
    Abort,
}

/// Parses incoming bytes, validates HMAC-signed commands, and returns a response with the action to take.
///
/// # Arguments
///
/// * `data` - Raw request bytes received over TCP.
/// * `config` - Shared service configuration including the secret and shutdown command.
/// * `peer_addr` - String representation of the client's address for logging.
///
/// # Returns
///
/// A tuple `(response, action)`, where `response` is sent back to the client, and
/// `action` indicates what the agent should do next.
///
/// # Panics
///
/// Panics if `config.shared_secret` is `None`. The shared secret should be set during
/// service initialization.
///
/// # Examples
///
/// ```
/// # use shuthost_host_agent::validation::validate_request;
/// # use shuthost_common::create_signed_message;
/// # use shuthost_host_agent::server::ServiceOptions;
/// # use shuthost_host_agent::validation::Action;
/// # use secrecy::SecretString;
///
/// let secret = SecretString::from("secret");
/// # let args = ServiceOptions { port: 0, shutdown_command: "cmd".to_string(), shared_secret: Some(secret.clone()) };
/// let signed = create_signed_message("status", &secret);
/// let (resp, action) = validate_request(signed.as_bytes(), &args, "peer");
/// assert_eq!(resp, "OK: status");
/// assert_eq!(action, Action::None);
/// ```
pub fn validate_request(data: &[u8], config: &ServiceOptions, peer_addr: &str) -> (String, Action) {
    let data_str = match std::str::from_utf8(data) {
        Ok(s) => s,
        Err(_) => {
            eprintln!("Invalid UTF-8 in request from {peer_addr}: {data:?}");
            return ("ERROR: Invalid UTF-8".to_string(), Action::None);
        }
    };

    match validate_hmac_message(
        data_str,
        config.shared_secret.as_ref().expect("Should be set by now"),
    ) {
        shuthost_common::HmacValidationResult::Valid(command) => {
            // Proceed with valid command
            match command.as_str() {
                "status" => ("OK: status".to_string(), Action::None),
                "shutdown" => (
                    format!(
                        "Now executing command: {}. Hopefully goodbye.",
                        config.shutdown_command
                    ),
                    Action::Shutdown,
                ),
                "abort" => ("OK: aborting service".to_string(), Action::Abort),
                _ => {
                    eprintln!("Invalid command from {peer_addr}: {command}");
                    ("ERROR: Invalid command".to_string(), Action::None)
                }
            }
        }
        shuthost_common::HmacValidationResult::InvalidTimestamp => {
            eprintln!("Timestamp out of range from {peer_addr}");
            ("ERROR: Timestamp out of range".to_string(), Action::None)
        }
        shuthost_common::HmacValidationResult::InvalidHmac => {
            eprintln!("Invalid HMAC signature from {peer_addr}");
            ("ERROR: Invalid HMAC signature".to_string(), Action::None)
        }
        shuthost_common::HmacValidationResult::MalformedMessage => {
            eprintln!("Invalid request format from {peer_addr}");
            ("ERROR: Invalid request format".to_string(), Action::None)
        }
    }
}

#[cfg(test)]
mod tests {
    use secrecy::SecretString;

    use super::*;
    use crate::server::ServiceOptions;

    fn make_args(secret: SecretString) -> ServiceOptions {
        ServiceOptions {
            port: 0,
            shutdown_command: "shutdown_cmd".to_string(),
            shared_secret: Some(secret),
        }
    }

    #[test]
    fn test_handle_invalid_utf8() {
        let args = make_args(SecretString::from("s"));
        let data = [0xff, 0xfe, 0xfd];
        let (resp, action) = validate_request(&data, &args, "peer");
        assert_eq!(resp, "ERROR: Invalid UTF-8");
        assert_eq!(action, Action::None);
    }

    #[test]
    fn test_handle_status() {
        let secret = SecretString::from("sec");
        let args = make_args(secret.clone());
        // create valid status command
        let signed = shuthost_common::create_signed_message("status", &secret);
        let (resp, action) = validate_request(signed.as_bytes(), &args, "peer");
        assert_eq!(resp, "OK: status");
        assert_eq!(action, Action::None);
    }

    #[test]
    fn test_handle_shutdown() {
        let secret = SecretString::from("sec");
        let args = make_args(secret.clone());
        let signed = shuthost_common::create_signed_message("shutdown", &secret);
        let (resp, action) = validate_request(signed.as_bytes(), &args, "peer");
        assert!(resp.contains("shutdown_cmd"));
        assert_eq!(action, Action::Shutdown);
    }

    #[test]
    fn test_handle_abort() {
        let secret = SecretString::from("sec");
        let args = make_args(secret.clone());
        let signed = shuthost_common::create_signed_message("abort", &secret);
        let (resp, action) = validate_request(signed.as_bytes(), &args, "peer");
        assert_eq!(resp, "OK: aborting service");
        assert_eq!(action, Action::Abort);
    }

    #[test]
    fn test_handle_invalid_timestamp() {
        let secret = SecretString::from("s");
        let args = make_args(secret);
        let data = "0|cmd|signature".to_string();
        let (resp, action) = validate_request(data.as_bytes(), &args, "peer");
        assert_eq!(resp, "ERROR: Timestamp out of range");
        assert_eq!(action, Action::None);
    }

    #[test]
    fn test_handle_invalid_hmac() {
        let secret = SecretString::from("s");
        let args = make_args(secret.clone());
        let signed = shuthost_common::create_signed_message("cmd", &secret) + "x";
        let (resp, action) = validate_request(signed.as_bytes(), &args, "peer");
        assert_eq!(resp, "ERROR: Invalid HMAC signature");
        assert_eq!(action, Action::None);
    }

    #[test]
    fn test_handle_malformed() {
        let secret = SecretString::from("s");
        let args = make_args(secret);
        let data = "no separators";
        let (resp, action) = validate_request(data.as_bytes(), &args, "peer");
        assert_eq!(resp, "ERROR: Invalid request format");
        assert_eq!(action, Action::None);
    }
}
