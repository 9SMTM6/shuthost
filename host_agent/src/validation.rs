//! Request validation utilities for the host agent.
//!
//! This module provides functions for parsing and validating
//! incoming HMAC-signed requests from the coordinator.

use core::str::{self, FromStr as _};

use crate::server::ServiceOptions;
use shuthost_common::{CoordinatorMessage, validate_hmac_message};

/// Parses incoming bytes, validates HMAC-signed commands, and returns the action to take or an error.
///
/// # Arguments
///
/// * `data` - Raw request bytes received over TCP.
/// * `config` - Shared service configuration including the secret and shutdown command.
/// * `peer_addr` - String representation of the client's address for logging.
///
/// # Returns
///
/// `Ok(action)` where `action` is `Some(CoordinatorMessage)` if an action is required, or `None` for status.
///
/// # Errors
///
/// For validation or parsing errors.
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
/// # use shuthost_common::CoordinatorMessage;
/// # use secrecy::SecretString;
///
/// let secret = SecretString::from("secret");
/// # let args = ServiceOptions { port: 0, shutdown_command: "cmd".to_string(), shared_secret: Some(secret.clone()), hostname: "test".to_string() };
/// let signed = create_signed_message("status", &secret);
/// let result = validate_request(signed.as_bytes(), &args);
/// assert_eq!(result, Ok(CoordinatorMessage::Status));
/// ```
pub fn validate_request(
    data: &[u8],
    config: &ServiceOptions,
) -> Result<CoordinatorMessage, &'static str> {
    let Ok(data_str) = str::from_utf8(data) else {
        return Err("Invalid UTF-8");
    };

    match validate_hmac_message(
        data_str,
        config.shared_secret.as_ref().expect("Should be set by now"),
    ) {
        shuthost_common::HmacValidationResult::Valid(command) => {
            use CoordinatorMessage as M;
            let Ok(msg): Result<M, _> = CoordinatorMessage::from_str(&command) else {
                return Err("Invalid command");
            };
            Ok(msg)
        }
        shuthost_common::HmacValidationResult::InvalidTimestamp => Err("Timestamp out of range"),
        shuthost_common::HmacValidationResult::InvalidHmac => Err("Invalid HMAC signature"),
        shuthost_common::HmacValidationResult::MalformedMessage => Err("Invalid request format"),
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
            hostname: "test_hostname".to_string(),
        }
    }

    #[test]
    fn test_handle_invalid_utf8() {
        let args = make_args(SecretString::from("s"));
        let data = [0xff, 0xfe, 0xfd];
        let result = validate_request(&data, &args);
        assert_eq!(result, Err("Invalid UTF-8"));
    }

    #[test]
    fn test_handle_status() {
        let secret = SecretString::from("sec");
        let args = make_args(secret.clone());
        // create valid status command
        let signed = shuthost_common::create_signed_message("status", &secret);
        let result = validate_request(signed.as_bytes(), &args);
        assert_eq!(result, Ok(CoordinatorMessage::Status));
    }

    #[test]
    fn test_handle_shutdown() {
        let secret = SecretString::from("sec");
        let args = make_args(secret.clone());
        let signed = shuthost_common::create_signed_message("shutdown", &secret);
        let result = validate_request(signed.as_bytes(), &args);
        assert_eq!(result, Ok(CoordinatorMessage::Shutdown));
    }

    #[test]
    fn test_handle_abort() {
        let secret = SecretString::from("sec");
        let args = make_args(secret.clone());
        let signed = shuthost_common::create_signed_message("abort", &secret);
        let result = validate_request(signed.as_bytes(), &args);
        assert_eq!(result, Ok(CoordinatorMessage::Abort));
    }

    #[test]
    fn test_handle_invalid_timestamp() {
        let secret = SecretString::from("s");
        let args = make_args(secret);
        let data = "0|cmd|signature".to_string();
        let result = validate_request(data.as_bytes(), &args);
        assert_eq!(result, Err("Timestamp out of range"));
    }

    #[test]
    fn test_handle_invalid_hmac() {
        let secret = SecretString::from("s");
        let args = make_args(secret.clone());
        let signed = shuthost_common::create_signed_message("cmd", &secret) + "x";
        let result = validate_request(signed.as_bytes(), &args);
        assert_eq!(result, Err("Invalid HMAC signature"));
    }

    #[test]
    fn test_handle_malformed() {
        let secret = SecretString::from("s");
        let args = make_args(secret);
        let data = "no separators";
        let result = validate_request(data.as_bytes(), &args);
        assert_eq!(result, Err("Invalid request format"));
    }
}
