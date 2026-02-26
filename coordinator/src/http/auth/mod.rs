//! Authentication for the coordinator: optional Token or OIDC based login.
//!
//! - Token mode: static token, generated if not provided. No bearer Token, token is only exchanged for a session cookie.
//! - OIDC mode: standard authorization code flow with PKCE. Maintains a signed
//!   session cookie once the user is authenticated.

pub mod cookies;
pub mod middleware;
pub mod oidc;
pub mod token;

use alloc::sync::Arc;

use crate::{
    app::state::AppState,
    config::OidcConfig,
    http::auth::oidc::{OidcClientReady, build_client},
};
use axum::extract::FromRef;
use axum::response::Redirect;
use axum_extra::extract::cookie::Key;
use base64::{Engine as _, engine::general_purpose::STANDARD as base64_gp_STANDARD};
use eyre::Context as _;
use secrecy::{ExposeSecret as _, SecretString};
use tokio::sync::RwLock;
use tracing::{Instrument as _, info, warn};

use crate::{
    app::db::{DbPool, KV_AUTH_TOKEN, KV_COOKIE_SECRET, delete_kv, get_kv, store_kv},
    config::{AuthConfig, AuthMode},
};

pub(crate) use cookies::{
    COOKIE_NONCE, COOKIE_OIDC_SESSION, COOKIE_PKCE, COOKIE_STATE, OIDCSessionClaims,
};
pub(crate) use middleware::{request_is_secure, require};

// Centralized login error keys used as query values on /login?error=<key>
pub(crate) const LOGIN_ERROR_INSECURE: &str = "insecure";
pub(crate) const LOGIN_ERROR_UNKNOWN: &str = "unknown";
pub(crate) const LOGIN_ERROR_TOKEN: &str = "token";
pub(crate) const LOGIN_ERROR_OIDC: &str = "oidc";
pub(crate) const LOGIN_ERROR_SESSION_EXPIRED: &str = "session_expired";

// Helper function for login error redirects
pub(crate) fn login_error_redirect(error: &str) -> Redirect {
    Redirect::to(&format!("/login?error={error}"))
}

pub(crate) struct Runtime {
    pub mode: Resolved,
    pub cookie_key: Key,
}

/// Shared (async) lock around the runtime OIDC client so it can be rebuilt on the fly when
/// discovery or key material changes.
pub(crate) type SharedOidcClient = Arc<RwLock<OidcClientReady>>;

#[derive(Debug)]
pub(crate) enum Resolved {
    Disabled,
    Token {
        token: Arc<SecretString>,
    },
    /// Resolved OIDC mode. The `config` field retains the original values from
    /// configuration so the client can be rebuilt on demand (e.g. when a
    /// discovery failure triggers a refresh).
    Oidc {
        client: SharedOidcClient,
        config: OidcConfig,
    },
    /// External auth (reverse proxy / external provider) acknowledged by operator.
    External {
        exceptions_version: u32,
    },
}

async fn get_or_generate_cookie_key(db_pool: Option<&DbPool>) -> eyre::Result<Key> {
    // small helpers to avoid repetition
    async fn gen_and_store_key(pool: &DbPool) -> eyre::Result<Key> {
        let generated = Key::generate();
        let encoded = base64_gp_STANDARD.encode(generated.master());
        store_kv(pool, KV_COOKIE_SECRET, &encoded).await?;
        Ok(generated)
    }

    // No configured secret - try database, then generate
    Ok(if let Some(pool) = db_pool {
        if let Some(stored_secret) = get_kv(pool, KV_COOKIE_SECRET).await? {
            // Try to decode stored secret
            match base64_gp_STANDARD.decode(&stored_secret) {
                Ok(bytes) => match Key::try_from(bytes.as_slice()) {
                    Ok(key) => key,
                    Err(_) => {
                        warn!(
                            "Found corrupted cookie key in DB (wrong length); removing and regenerating"
                        );
                        delete_kv(pool, KV_COOKIE_SECRET).await?;
                        gen_and_store_key(pool).await?
                    }
                },
                Err(_) => {
                    warn!(
                        "Found corrupted cookie key in DB (invalid base64); removing and regenerating"
                    );
                    delete_kv(pool, KV_COOKIE_SECRET).await?;
                    gen_and_store_key(pool).await?
                }
            }
        } else {
            // No stored value - generate and store
            gen_and_store_key(pool).await?
        }
    } else {
        // No database - generate without storing
        Key::generate()
    })
}

impl Runtime {
    /// Creates a new `Runtime` instance from the provided configuration.
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// - The configured `cookie_secret` is not valid base64
    /// - The configured `cookie_secret` does not decode to exactly 32 bytes
    /// - Database operations fail when storing, retrieving, or deleting cookie secrets or auth tokens
    /// - A stored cookie secret in the database is corrupted (invalid base64 or wrong length)
    pub(crate) async fn from_config(
        cfg: &AuthConfig,
        db_pool: Option<&DbPool>,
    ) -> eyre::Result<Self> {
        let cookie_key = setup_cookie_key(cfg.cookie_secret.as_ref(), db_pool).await?;
        let mode = resolve_auth_mode(&cfg.mode, db_pool).await?;

        Ok(Self { mode, cookie_key })
    }
}

/// Set up the cookie key from config or database.
#[tracing::instrument(skip_all)]
async fn setup_cookie_key(
    cookie_secret: Option<&Arc<SecretString>>,
    db_pool: Option<&DbPool>,
) -> eyre::Result<Key> {
    if let Some(cookie_secret_val) = cookie_secret {
        // Configured cookie secret - remove any stored value to avoid confusion
        if let Some(pool) = db_pool {
            info!(
                "Configured cookie_secret present in config; removing any stored cookie_secret from DB to avoid confusion"
            );
            delete_kv(pool, KV_COOKIE_SECRET).await?;
        }

        // Try to decode the configured secret
        let bytes = base64_gp_STANDARD
            .decode((*cookie_secret_val).expose_secret().as_bytes())
            .wrap_err("Invalid cookie_secret in config")?;
        Key::try_from(bytes.as_slice())
            .wrap_err("Invalid cookie_secret length in config: expected 32 bytes")
    } else {
        get_or_generate_cookie_key(db_pool).await
    }
}

/// Resolve the authentication mode from configuration.
#[tracing::instrument(skip(db_pool), ret)]
async fn resolve_auth_mode(mode: &AuthMode, db_pool: Option<&DbPool>) -> eyre::Result<Resolved> {
    match *mode {
        AuthMode::None => Ok(Resolved::Disabled),
        AuthMode::Token { ref token } => {
            resolve_token_auth(token.as_ref(), db_pool)
                .in_current_span()
                .await
        }
        AuthMode::Oidc(ref oidc_cfg) => {
            let client_inner = build_client(
                &oidc_cfg.issuer,
                &oidc_cfg.client_id,
                &oidc_cfg.client_secret,
            )
            .in_current_span()
            .await
            .wrap_err("Failed to build OIDC client")?;
            let client = Arc::new(RwLock::new(client_inner));
            Ok(Resolved::Oidc {
                client,
                config: oidc_cfg.clone(),
            })
        }
        AuthMode::External { exceptions_version } => Ok(Resolved::External { exceptions_version }),
    }
}

/// Resolve token authentication mode.
async fn resolve_token_auth(
    config_token: Option<&Arc<SecretString>>,
    db_pool: Option<&DbPool>,
) -> eyre::Result<Resolved> {
    let token = if let Some(cfg_token) = config_token {
        // Configured token - remove any stored value to avoid confusion
        if let Some(pool) = db_pool {
            delete_kv(pool, KV_AUTH_TOKEN).await?;
        }
        cfg_token.clone()
    } else {
        resolve_auto_token(db_pool).in_current_span().await?
    };

    Ok(Resolved::Token { token })
}

/// Resolve token when not configured (try DB, then generate).
async fn resolve_auto_token(db_pool: Option<&DbPool>) -> eyre::Result<Arc<SecretString>> {
    if let Some(pool) = db_pool {
        if let Some(stored_token) = get_kv(pool, KV_AUTH_TOKEN).await? {
            info!("Auth mode: token (from database)");
            Ok(Arc::new(SecretString::from(stored_token)))
        } else {
            let generated = cookies::generate_token();
            store_kv(pool, KV_AUTH_TOKEN, generated.expose_secret()).await?;
            info!("Auth mode: token (auto generated, stored in db)");
            // We expose the generated token in logs once for operator use
            info!("Token: {}", generated.expose_secret());
            Ok(generated)
        }
    } else {
        let generated = cookies::generate_token();
        info!("Auth mode: token (auto generated, not stored for lack of a db)");
        // We expose the generated token in logs once for operator use
        info!("Token: {}", generated.expose_secret());
        Ok(generated)
    }
}

#[derive(Clone)]
pub(crate) struct LayerState {
    pub auth: Arc<Runtime>,
}

impl FromRef<AppState> for LayerState {
    fn from_ref(input: &AppState) -> Self {
        Self {
            auth: input.auth.clone(),
        }
    }
}

impl FromRef<AppState> for Key {
    fn from_ref(input: &AppState) -> Self {
        input.auth.cookie_key.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::db;
    use crate::config::AuthConfig;
    use std::path::Path;

    async fn setup_db() -> eyre::Result<DbPool> {
        db::init(Path::new(":memory:")).await
    }

    #[tokio::test]
    async fn removes_db_values_when_configured() {
        let pool = setup_db().await.unwrap();

        // store values in DB
        db::store_kv(&pool, KV_AUTH_TOKEN, "from_db").await.unwrap();
        db::store_kv(
            &pool,
            KV_COOKIE_SECRET,
            &base64_gp_STANDARD.encode(Key::generate().master()),
        )
        .await
        .unwrap();

        let cfg_token = Some(Arc::new(SecretString::from("configured_token")));

        // Provide configured values -> they should cause DB entries to be removed
        let cfg = AuthConfig {
            mode: AuthMode::Token {
                token: cfg_token.clone(),
            },
            cookie_secret: Some(Arc::new(SecretString::from(
                base64_gp_STANDARD.encode(Key::generate().master()),
            ))),
        };

        let runtime = Runtime::from_config(&cfg, Some(&pool)).await.unwrap();

        // DB entries should be removed
        assert!(db::get_kv(&pool, KV_AUTH_TOKEN).await.unwrap().is_none());
        assert!(db::get_kv(&pool, KV_COOKIE_SECRET).await.unwrap().is_none());

        // runtime should use configured token
        match runtime.mode {
            Resolved::Token { token } => assert_eq!((*token).expose_secret(), "configured_token"),
            _ => panic!("expected token mode"),
        }
    }

    #[tokio::test]
    async fn invalid_configured_cookie_secret_fails() {
        let pool = setup_db().await.unwrap();

        // invalid base64 value in config
        let cfg = AuthConfig {
            mode: AuthMode::None,
            cookie_secret: Some(Arc::new(SecretString::from("not-base64!!"))),
        };

        let res = Runtime::from_config(&cfg, Some(&pool)).await;
        assert!(res.is_err());
    }
}
