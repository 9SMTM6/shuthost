//! Authentication for the coordinator: optional Token or OIDC based login.
//!
//! - Token mode: static bearer token, generated if not provided. Expects a cookie set via the built-in /login form.
//! - OIDC mode: standard authorization code flow with PKCE. Maintains a signed
//!   session cookie once the user is authenticated.

mod cookies;
mod middleware;
mod oidc;
mod routes;
mod token;

use std::sync::Arc;

use axum::extract::FromRef;
use axum_extra::extract::cookie::Key;
use tracing::info;

use crate::{
    config::{AuthConfig, AuthMode, ControllerConfig},
    http::AppState,
};

pub use cookies::{COOKIE_NONCE, COOKIE_OIDC_SESSION, COOKIE_PKCE, COOKIE_RETURN_TO, COOKIE_STATE};
pub use middleware::{request_is_secure, require};
pub use routes::{EXPECTED_EXCEPTIONS_VERSION, OIDCSessionClaims, public_routes};

// Centralized login error keys used as query values on /login?error=<key>
pub const LOGIN_ERROR_INSECURE: &str = "insecure";
pub const LOGIN_ERROR_UNKNOWN: &str = "unknown";
pub const LOGIN_ERROR_TOKEN: &str = "token";
pub const LOGIN_ERROR_OIDC: &str = "oidc";
pub const LOGIN_ERROR_SESSION_EXPIRED: &str = "session_expired";

#[derive(Clone)]
pub struct Runtime {
    pub mode: Resolved,
    pub cookie_key: Key,
}

#[derive(Clone, Debug)]
pub enum Resolved {
    Disabled,
    Token {
        token: String,
    },
    Oidc {
        issuer: String,
        client_id: String,
        client_secret: String,
        scopes: Vec<String>,
    },
    /// External auth (reverse proxy / external provider) acknowledged by operator.
    External {
        exceptions_version: u32,
    },
}

impl Runtime {
    pub fn from_config(cfg: &ControllerConfig) -> Self {
        let (mode, cookie_key) = match cfg.server.auth {
            AuthConfig {
                mode: AuthMode::None,
                ref cookie_secret,
                ..
            } => (
                Resolved::Disabled,
                cookies::key_from_secret(cookie_secret.as_deref()),
            ),
            AuthConfig {
                mode: AuthMode::Token { ref token },
                ref cookie_secret,
                ..
            } => {
                // If a token was configured in the TOML config, don't log its value
                // (it is already present in the config file). Only log the token
                // value when we auto-generate one on startup so operators can copy it.
                let token = if let Some(cfg_token) = token.clone() {
                    info!("Auth mode: token (configured)");
                    cfg_token
                } else {
                    let generated = cookies::generate_token();
                    info!("Auth mode: token");
                    info!("Token: {}", generated);
                    generated
                };

                (
                    Resolved::Token { token },
                    cookies::key_from_secret(cookie_secret.as_deref()),
                )
            }
            AuthConfig {
                mode:
                    AuthMode::Oidc {
                        ref issuer,
                        ref client_id,
                        ref client_secret,
                        ref scopes,
                    },
                ref cookie_secret,
            } => {
                info!("Auth mode: oidc, issuer={}", issuer);
                (
                    Resolved::Oidc {
                        issuer: issuer.clone(),
                        client_id: client_id.clone(),
                        client_secret: client_secret.clone(),
                        scopes: scopes.clone(),
                    },
                    cookies::key_from_secret(cookie_secret.as_deref()),
                )
            }
            AuthConfig {
                mode: AuthMode::External { exceptions_version },
                ref cookie_secret,
            } => {
                info!("Auth mode: external (reverse proxy)");
                (
                    Resolved::External { exceptions_version },
                    cookies::key_from_secret(cookie_secret.as_deref()),
                )
            }
        };
        Self { mode, cookie_key }
    }
}

#[derive(Clone)]
pub struct LayerState {
    pub auth: Arc<Runtime>,
}

impl FromRef<AppState> for LayerState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            auth: state.auth.clone(),
        }
    }
}

impl FromRef<AppState> for Key {
    fn from_ref(state: &AppState) -> Self {
        state.auth.cookie_key.clone()
    }
}
