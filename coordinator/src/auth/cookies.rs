//! Cookie handling utilities for authentication.

use axum_extra::extract::{
    SignedCookieJar,
    cookie::{Cookie, Key},
};
use base64::Engine;
use cookie::{SameSite, time::Duration as CookieDuration};
use rand::{Rng as _, distr::Alphanumeric};

use crate::auth::{OIDCSessionClaims, routes::TokenSessionClaims};

/// Cookie name constants for authentication
pub const COOKIE_OIDC_SESSION: &str = "shuthost_oidc_session";
pub const COOKIE_TOKEN_SESSION: &str = "shuthost_token_session";
pub const COOKIE_STATE: &str = "shuthost_oidc_state";
pub const COOKIE_NONCE: &str = "shuthost_oidc_nonce";
pub const COOKIE_PKCE: &str = "shuthost_oidc_pkce";
pub const COOKIE_RETURN_TO: &str = "shuthost_return_to";

/// Generate a cookie key from an optional base64-encoded secret string.
/// Falls back to generating a random key if the secret is invalid or missing.
pub fn key_from_secret(secret: Option<&str>) -> Key {
    secret
        .and_then(|s| base64::engine::general_purpose::STANDARD.decode(s).ok())
        .and_then(|bytes| Key::try_from(&bytes[..]).ok())
        .unwrap_or_else(Key::generate)
}

/// Generate a random alphanumeric token of 48 characters.
pub fn generate_token() -> String {
    rand::rng()
        .sample_iter(Alphanumeric)
        .take(48)
        .map(char::from)
        .collect()
}

/// Create a return-to cookie for redirect-after-login functionality.
pub fn create_return_to_cookie(return_to: String) -> Cookie<'static> {
    Cookie::build((COOKIE_RETURN_TO, return_to))
        .http_only(true)
        .secure(true)
        .same_site(SameSite::Strict)
        .max_age(CookieDuration::minutes(10))
        .path("/")
        .build()
}

/// Create a token cookie for authentication.
pub fn create_token_session_cookie(
    token_data: &TokenSessionClaims,
    session_max_age: CookieDuration,
) -> Cookie<'static> {
    Cookie::build((
        COOKIE_TOKEN_SESSION,
        serde_json::to_string(&token_data).unwrap(),
    ))
    .http_only(true)
    .secure(true)
    .same_site(SameSite::Strict)
    .max_age(session_max_age)
    .path("/")
    .build()
}

/// Create a session cookie for OIDC authentication.
pub fn create_oidc_session_cookie(
    session_data: &OIDCSessionClaims,
    session_max_age: CookieDuration,
) -> Cookie<'static> {
    Cookie::build((
        COOKIE_OIDC_SESSION,
        serde_json::to_string(session_data).unwrap(),
    ))
    .http_only(true)
    .secure(true)
    .same_site(SameSite::Strict)
    .max_age(session_max_age)
    .path("/")
    .build()
}

pub fn get_oidc_session_from_cookie(signed: &SignedCookieJar) -> Option<OIDCSessionClaims> {
    signed
        .get(COOKIE_OIDC_SESSION)
        .and_then(|session| serde_json::from_str::<OIDCSessionClaims>(session.value()).ok())
}

pub fn get_token_session_from_cookie(signed: &SignedCookieJar) -> Option<TokenSessionClaims> {
    signed
        .get(COOKIE_TOKEN_SESSION)
        .and_then(|session| serde_json::from_str::<TokenSessionClaims>(session.value()).ok())
}
