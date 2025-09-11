//! Authentication for the coordinator: optional Token or OIDC based login.
//!
//! - Token mode: static bearer token, generated if not provided. Expects a cookie set via the built-in /login form.
//! - OIDC mode: standard authorization code flow with PKCE. Maintains a signed
//!   session cookie once the user is authenticated.

// TODO:
// 2) remove token header auth, unless theres a good reason for it
// 3) Check back on logout button issue with oidc (prompt=login), doesnt seem to be fixed.
//  ==> kanidm doesnt support prompt=login, need alternative for at least it.
// 4) token auth doesnt work properly right now
// 5) why is it possible to set redirect_path at all? seems like it has to fit to the app anyways, so should be fixed.

use axum::{
    Router,
    body::Body,
    extract::{FromRef, State},
    http::{HeaderMap, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
};
use axum_extra::extract::cookie::{Cookie, Key, SignedCookieJar};
use base64::Engine;
use rand::{Rng as _, distr::Alphanumeric};
use serde::{Deserialize, Serialize};
use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tracing::info;

use crate::config::{AuthConfig, AuthMode, ControllerConfig};
use crate::http::AppState;

const COOKIE_SESSION: &str = "shuthost_session";
const COOKIE_TOKEN: &str = "shuthost_token";
const COOKIE_STATE: &str = "shuthost_oidc_state";
const COOKIE_NONCE: &str = "shuthost_oidc_nonce";
const COOKIE_PKCE: &str = "shuthost_oidc_pkce";
const COOKIE_RETURN_TO: &str = "shuthost_return_to";
const COOKIE_LOGGED_OUT: &str = "shuthost_logged_out";

mod oidc;
mod token;

#[derive(Clone)]
pub struct AuthRuntime {
    pub mode: AuthResolved,
    pub cookie_key: Key,
}

#[derive(Clone, Debug)]
pub enum AuthResolved {
    Disabled,
    Token {
        token: String,
    },
    Oidc {
        issuer: String,
        client_id: String,
        client_secret: String,
        scopes: Vec<String>,
        redirect_path: String, // path only, build full URL from Host header
    },
}

impl AuthRuntime {
    pub fn from_config(cfg: &ControllerConfig) -> Self {
        let (mode, cookie_key) = match cfg.server.auth {
            AuthConfig {
                mode: AuthMode::None,
                ref cookie_secret,
                ..
            } => (
                AuthResolved::Disabled,
                key_from_secret(cookie_secret.as_deref()),
            ),
            AuthConfig {
                mode: AuthMode::Token { ref token },
                ref cookie_secret,
                ..
            } => {
                let token = token.clone().unwrap_or_else(generate_token);
                info!("Auth mode: token");
                info!("Token: {}", token);
                (
                    AuthResolved::Token { token },
                    key_from_secret(cookie_secret.as_deref()),
                )
            }
            AuthConfig {
                mode:
                    AuthMode::Oidc {
                        ref issuer,
                        ref client_id,
                        ref client_secret,
                        ref scopes,
                        ref redirect_path,
                    },
                ref cookie_secret,
            } => {
                info!("Auth mode: oidc, issuer={}", issuer);
                (
                    AuthResolved::Oidc {
                        issuer: issuer.clone(),
                        client_id: client_id.clone(),
                        client_secret: client_secret.clone(),
                        scopes: scopes.clone(),
                        redirect_path: redirect_path.clone(),
                    },
                    key_from_secret(cookie_secret.as_deref()),
                )
            }
        };
        Self { mode, cookie_key }
    }
}

fn key_from_secret(secret: Option<&str>) -> Key {
    secret
        .and_then(|s| base64::engine::general_purpose::STANDARD.decode(s).ok())
        .and_then(|bytes| Key::try_from(&bytes[..]).ok())
        .unwrap_or_else(Key::generate)
}

fn generate_token() -> String {
    rand::rng()
        .sample_iter(Alphanumeric)
        .take(48)
        .map(char::from)
        .collect()
}

#[derive(Clone)]
pub struct AuthLayerState {
    pub auth: Arc<AuthRuntime>,
}

impl FromRef<AppState> for AuthLayerState {
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

pub fn public_routes() -> Router<AppState> {
    use crate::routes::{get_download_router, m2m_routes};

    Router::new()
        // Auth endpoints
        .route("/login", get(token::login_get).post(token::login_post))
        .route("/logout", post(logout))
        .route("/oidc/login", get(oidc::oidc_login))
        .route("/oidc/callback", get(oidc::oidc_callback))
        // PWA & static assets bundled via asset_routes
        .merge(crate::assets::asset_routes())
        // Bypass routes
        .nest("/download", get_download_router())
        .nest("/api/m2m", m2m_routes())
}

/// Middleware that enforces auth depending on configured mode.
pub async fn require_auth(
    State(AuthLayerState { auth }): State<AuthLayerState>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let headers = req.headers();
    match auth.mode {
        AuthResolved::Disabled => next.run(req).await,
        AuthResolved::Token { ref token } => {
            let cookie_ok = get_cookie(headers, COOKIE_TOKEN)
                .map(|v| v == *token)
                .unwrap_or(false);
            tracing::debug!(cookie_ok, "require_auth: token cookie check");
            if cookie_ok {
                next.run(req).await
            } else if wants_html(headers) {
                // remember path for redirect-after-login
                let return_to = req.uri().to_string();
                tracing::info!(return_to = %return_to, "require_auth: no token, redirecting to /login and setting return_to cookie");
                let jar = SignedCookieJar::from_headers(headers, auth.cookie_key.clone()).add(
                    Cookie::build((COOKIE_RETURN_TO, return_to))
                        .path("/")
                        .build(),
                );
                (jar, Redirect::temporary("/login")).into_response()
            } else {
                StatusCode::UNAUTHORIZED.into_response()
            }
        }
        AuthResolved::Oidc { .. } => {
            // Check signed session cookie via headers
            let signed = SignedCookieJar::from_headers(headers, auth.cookie_key.clone());
            if let Some(session) = signed.get(COOKIE_SESSION)
                && let Ok(sess) = serde_json::from_str::<SessionClaims>(session.value())
                && !sess.is_expired()
            {
                return next.run(req).await;
            }
            tracing::info!("require_auth: no valid session cookie, initiating OIDC login flow");
            if wants_html(headers) {
                let return_to = req.uri().to_string();
                tracing::info!(return_to = %return_to, "require_auth: setting return_to cookie and redirecting to /oidc/login");
                let jar = SignedCookieJar::from_headers(headers, auth.cookie_key.clone()).add(
                    Cookie::build((COOKIE_RETURN_TO, return_to))
                        .path("/")
                        .build(),
                );
                (jar, Redirect::temporary("/oidc/login")).into_response()
            } else {
                StatusCode::UNAUTHORIZED.into_response()
            }
        }
    }
}

fn wants_html(headers: &HeaderMap) -> bool {
    headers
        .get(axum::http::header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.contains("text/html"))
        .unwrap_or(false)
}

fn get_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    let header = headers.get(axum::http::header::COOKIE)?.to_str().ok()?;
    for pair in header.split(';') {
        let mut parts = pair.trim().splitn(2, '=');
        let k = parts.next()?;
        let v = parts.next().unwrap_or("");
        if k == name {
            return Some(v.to_string());
        }
    }
    None
}

async fn logout(jar: SignedCookieJar) -> impl IntoResponse {
    // Log what cookies we saw when logout was invoked so we can ensure the path is hit
    let had_session = jar.get(COOKIE_SESSION).is_some();
    let had_token = jar.get(COOKIE_TOKEN).is_some();
    let had_logged_out = jar.get(COOKIE_LOGGED_OUT).is_some();
    tracing::info!(
        had_session,
        had_token,
        had_logged_out,
        "logout: received request"
    );

    let jar = jar
        .remove(Cookie::build(COOKIE_TOKEN).path("/").build())
        .remove(Cookie::build(COOKIE_SESSION).path("/").build())
        // Mark that the user intentionally logged out to force interactive login on next OIDC auth
        .add(
            Cookie::build((COOKIE_LOGGED_OUT, "1"))
                .http_only(true)
                .path("/")
                .build(),
        );

    tracing::info!("logout: removed session/token and set logged_out cookie");
    (jar, Redirect::to("/login")).into_response()
}

#[derive(Serialize, Deserialize)]
struct SessionClaims {
    sub: String,
    exp: u64,
}
impl SessionClaims {
    fn is_expired(&self) -> bool {
        now_ts() >= self.exp
    }
}

fn now_ts() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn request_origin(headers: &HeaderMap) -> Option<String> {
    let host = headers
        .get("x-forwarded-host")
        .or_else(|| headers.get("host"))?
        .to_str()
        .ok()?;
    let proto = headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("http");
    Some(format!("{}://{}", proto, host))
}
