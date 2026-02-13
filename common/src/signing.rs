//! HMAC signing utilities for creating signed messages.
//!
//! This module provides functions for creating HMAC signatures and
//! formatting signed messages with timestamps.

use hmac::{Hmac, Mac};
use secrecy::ExposeSecret;
use sha2::Sha256;
use std::time::{SystemTime, UNIX_EPOCH};

/// Creates an HMAC instance for the given message and secret.
#[expect(
    clippy::missing_panics_doc,
    reason = "Expectation should never be false"
)]
#[must_use]
pub fn create_hmac(message: &str, secret: &[u8]) -> Hmac<Sha256> {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret).expect("HMAC can take a key of any size");
    mac.update(message.as_bytes());
    mac
}

/// Signs a message with HMAC using the provided secret.
#[must_use]
pub fn sign_hmac(message: &str, secret: &secrecy::SecretString) -> String {
    let mac = create_hmac(message, secret.expose_secret().as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

/// Creates a signed message by prepending a timestamp and appending an HMAC signature.
///
/// # Arguments
///
/// * `command` - The command to sign.
/// * `secret` - The secret key used for HMAC.
///
/// # Returns
///
/// A string of the form "timestamp|command|signature".
#[must_use]
pub fn create_signed_message(command: &str, secret: &secrecy::SecretString) -> String {
    let message = format!("{}|{}", unix_time_seconds(), command);
    format!("{}|{}", message, sign_hmac(&message, secret))
}

/// Gets the current Unix timestamp in seconds.
#[expect(
    clippy::missing_panics_doc,
    reason = "Expectation should never be false"
)]
#[must_use]
pub fn unix_time_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs()
}
