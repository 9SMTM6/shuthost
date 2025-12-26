//! Cookie handling utilities for authentication.

use axum_extra::extract::{SignedCookieJar, cookie::Cookie};
use cookie::{SameSite, time::Duration as CookieDuration};
use rand::{Rng as _, distr::Alphanumeric};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

/// Cookie name constants for authentication
pub(crate) const COOKIE_OIDC_SESSION: &str = "shuthost_oidc_session";
pub(crate) const COOKIE_TOKEN_SESSION: &str = "shuthost_token_session";
pub(crate) const COOKIE_STATE: &str = "shuthost_oidc_state";
pub(crate) const COOKIE_NONCE: &str = "shuthost_oidc_nonce";
pub(crate) const COOKIE_PKCE: &str = "shuthost_oidc_pkce";
const COOKIE_RETURN_TO: &str = "shuthost_return_to";

/// Session claims for token authentication.
#[derive(Serialize, Deserialize)]
pub(crate) struct TokenSessionClaims {
    pub iat: u64,           // issued at
    pub exp: u64,           // expiry
    pub token_hash: String, // hash of the token
}

impl TokenSessionClaims {
    pub(crate) fn new(token: &str) -> Self {
        let now = now_ts();
        let exp_duration: i64 = 60 * 60 * 8; // 8 hours expiry
        Self {
            iat: now,
            exp: now + exp_duration as u64,
            token_hash: {
                let mut hasher = Sha256::new();
                hasher.update(token.as_bytes());
                format!("{:x}", hasher.finalize())
            },
        }
    }

    /// Check if the session has expired.
    pub(crate) fn is_expired(&self) -> bool {
        now_ts() >= self.exp
    }
    /// Check if the token matches (by hash).
    pub(crate) fn matches_token(&self, token: &SecretString) -> bool {
        let mut hasher = Sha256::new();
        hasher.update(token.expose_secret().as_bytes());
        let hash = format!("{:x}", hasher.finalize());
        self.token_hash == hash
    }
}

/// Session claims for OIDC authentication.
/// Contains some claims from the [OIDC Id Token](https://openid.net/specs/openid-connect-core-1_0.html#IDToken)
#[derive(Serialize, Deserialize)]
pub(crate) struct OIDCSessionClaims {
    /// The sub claim, a unique user identifier
    pub sub: String,
    /// The expiry as provided by the IdP, after which shuthost should reject the session. Unix second timestamp
    pub exp: u64,
}

impl OIDCSessionClaims {
    /// Check if the session has expired.
    pub(crate) fn is_expired(&self) -> bool {
        now_ts() >= self.exp
    }
}

/// Get the current timestamp in seconds since UNIX epoch.
///
/// # Panics
///
/// Panics if the system time is set to before the UNIX epoch (January 1, 1970).
pub(crate) fn now_ts() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Invalidate session cookies for logout.
pub(crate) fn invalidate_session(jar: SignedCookieJar) -> SignedCookieJar {
    // Log what cookies we saw when logout was invoked so we can ensure the path is hit
    let had_session_oidc = jar.get(COOKIE_OIDC_SESSION).is_some();
    let had_session_token = jar.get(COOKIE_TOKEN_SESSION).is_some();
    tracing::info!(
        had_session_oidc,
        had_session_token,
        "logout: received request"
    );

    let jar = jar
        .remove(Cookie::build(COOKIE_TOKEN_SESSION).path("/").build())
        .remove(Cookie::build(COOKIE_OIDC_SESSION).path("/").build());

    tracing::info!("logout: removed session cookies");
    jar
}

/// Generate a random alphanumeric token of 48 characters.
pub(crate) fn generate_token() -> Arc<SecretString> {
    Arc::new(
        rand::rng()
            .sample_iter(Alphanumeric)
            .take(48)
            .map(char::from)
            .collect::<String>()
            .into(),
    )
}

/// Create a protected cookie with standard security properties.
pub(crate) fn create_protected_cookie(
    name: &'static str,
    value: String,
    max_age: CookieDuration,
) -> Cookie<'static> {
    Cookie::build((name, value))
        .http_only(true)
        .secure(true)
        .same_site(SameSite::Strict)
        .max_age(max_age)
        .path("/")
        .build()
}

/// Create a return-to cookie for redirect-after-login functionality.
pub(crate) fn create_return_to_cookie(return_to: String) -> Cookie<'static> {
    create_protected_cookie(COOKIE_RETURN_TO, return_to, CookieDuration::minutes(10))
}

/// Create a token cookie for authentication.
///
/// # Panics
///
/// Panics if the token data cannot be serialized to JSON.
pub(crate) fn create_token_session_cookie(
    token_data: &TokenSessionClaims,
    session_max_age: CookieDuration,
) -> Cookie<'static> {
    create_protected_cookie(
        COOKIE_TOKEN_SESSION,
        serde_json::to_string(token_data).unwrap(),
        session_max_age,
    )
}

/// Create a session cookie for OIDC authentication.
///
/// # Panics
///
/// Panics if the session data cannot be serialized to JSON.
pub(crate) fn create_oidc_session_cookie(
    session_data: &OIDCSessionClaims,
    session_max_age: CookieDuration,
) -> Cookie<'static> {
    create_protected_cookie(
        COOKIE_OIDC_SESSION,
        serde_json::to_string(session_data).unwrap(),
        session_max_age,
    )
}

pub(crate) fn get_oidc_session_from_cookie(jar: &SignedCookieJar) -> Option<OIDCSessionClaims> {
    jar.get(COOKIE_OIDC_SESSION)
        .and_then(|session| serde_json::from_str::<OIDCSessionClaims>(session.value()).ok())
}

pub(crate) fn get_token_session_from_cookie(jar: &SignedCookieJar) -> Option<TokenSessionClaims> {
    jar.get(COOKIE_TOKEN_SESSION)
        .and_then(|session| serde_json::from_str::<TokenSessionClaims>(session.value()).ok())
}

#[must_use]
pub(crate) fn extract_return_to_and_remove_cookie(jar: SignedCookieJar) -> (String, SignedCookieJar) {
    let return_to = jar
        .get(COOKIE_RETURN_TO)
        .map(|c| c.value().to_string())
        .unwrap_or_else(|| "/".to_string());
    let jar = jar.remove(Cookie::build(COOKIE_RETURN_TO).path("/").build());
    (return_to, jar)
}
