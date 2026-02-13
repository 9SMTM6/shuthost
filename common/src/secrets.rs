//! Secret generation utilities for HMAC keys.
//!
//! This module provides functions for generating cryptographically
//! secure random secrets suitable for use as HMAC keys.

use rand::Rng;

/// Generates a random secret string suitable for use as an HMAC key.
///
/// Returns a 32-character alphanumeric string.
#[must_use]
pub fn generate_secret() -> String {
    // Simple random secret generation: 32 characters
    let mut rng = rand::rng();
    (0..32)
        .map(|_| rng.sample(rand::distr::Alphanumeric) as char)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_secret() {
        let secret = generate_secret();
        assert_eq!(secret.len(), 32);
    }
}
