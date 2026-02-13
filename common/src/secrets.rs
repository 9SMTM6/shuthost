//! Secret generation utilities for HMAC keys.
//!
//! This module provides functions for generating cryptographically
//! secure random secrets suitable for use as HMAC keys.

use core::iter;
use rand::{Rng as _, distr, rng};

/// Generates a random secret string suitable for use as an HMAC key.
///
/// Returns a 32-character alphanumeric string.
#[must_use]
pub fn generate_secret() -> String {
    // Simple random secret generation: 32 characters
    let mut rng = rng();
    iter::repeat_with(|| rng.sample(distr::Alphanumeric) as char)
        .take(32)
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
