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
use base64::{Engine, engine::general_purpose::STANDARD as base64_gp_STANDARD};
use eyre::Context;
use tracing::info;

use crate::{
    config::{AuthConfig, AuthMode},
    db::{delete_kv, get_kv, store_kv, DbPool},
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
    pub async fn from_config(cfg: &AuthConfig, db_pool: Option<&DbPool>) -> eyre::Result<Self> {
        // Handle cookie key: configured takes precedence, stored is fallback
        let cookie_key = if let Some(cookie_secret) = &cfg.cookie_secret {
            // Configured cookie secret - remove any stored value to avoid confusion
            if let Some(pool) = db_pool {
                delete_kv(pool, "cookie_secret").await?;
            }
            
            // Try to decode the configured secret
            base64_gp_STANDARD.decode(cookie_secret)
                .wrap_err("Invalid cookie_secret in config")?
                .as_slice()
                .try_into()
                .wrap_err("Invalid cookie_secret length in config: expected 32 bytes")?
        } else {
            // No configured secret - try database, then generate
            if let Some(pool) = db_pool {
                if let Some(stored_secret) = get_kv(pool, "cookie_secret").await? {
                    // Try to decode stored secret
                    match base64_gp_STANDARD.decode(&stored_secret) {
                        Ok(bytes) => match Key::try_from(bytes.as_slice()) {
                            Ok(key) => key,
                            Err(_) => {
                                // Invalid stored key - remove it and generate new one
                                delete_kv(pool, "cookie_secret").await?;
                                let generated = Key::generate();
                                let encoded = base64_gp_STANDARD.encode(generated.master());
                                store_kv(pool, "cookie_secret", &encoded).await?;
                                generated
                            }
                        },
                        Err(_) => {
                            // Invalid base64 in stored value - remove it and generate new one
                            delete_kv(pool, "cookie_secret").await?;
                            let generated = Key::generate();
                            let encoded = base64_gp_STANDARD.encode(generated.master());
                            store_kv(pool, "cookie_secret", &encoded).await?;
                            generated
                        }
                    }
                } else {
                    // No stored value - generate and store
                    let generated = Key::generate();
                    let encoded = base64_gp_STANDARD.encode(generated.master());
                    store_kv(pool, "cookie_secret", &encoded).await?;
                    generated
                }
            } else {
                // No database - generate without storing
                Key::generate()
            }
        };
        
        let mode = match cfg.mode {
            AuthMode::None => Resolved::Disabled,
            AuthMode::Token { ref token } => {
                let token = if let Some(cfg_token) = token.clone() {
                    // Configured token - remove any stored value to avoid confusion
                    if let Some(pool) = db_pool {
                        delete_kv(pool, "auth_token").await?;
                    }
                    info!("Auth mode: token (configured)");
                    cfg_token
                } else {
                    // No configured token - try database, then generate
                    if let Some(pool) = db_pool {
                        if let Some(stored_token) = get_kv(pool, "auth_token").await? {
                            info!("Auth mode: token (from database)");
                            stored_token
                        } else {
                            let generated = cookies::generate_token();
                            info!("Auth mode: token (auto generated, stored in db)");
                            info!("Token: {}", generated);
                            store_kv(pool, "auth_token", &generated).await?;
                            generated
                        }
                    } else {
                        let generated = cookies::generate_token();
                        info!("Auth mode: token (auto generated, not stored for lack of a db)");
                        info!("Token: {}", generated);
                        generated
                    }
                };

                Resolved::Token { token }
            }
            AuthMode::Oidc {
                ref issuer,
                ref client_id,
                ref client_secret,
                ref scopes,
            } => {
                info!("Auth mode: oidc, issuer={}", issuer);
                Resolved::Oidc {
                    issuer: issuer.clone(),
                    client_id: client_id.clone(),
                    client_secret: client_secret.clone(),
                    scopes: scopes.clone(),
                }
            }
            AuthMode::External { exceptions_version } => {
                info!("Auth mode: external (reverse proxy)");
                Resolved::External { exceptions_version }
            }
        };
        Ok(Self { mode, cookie_key })
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
