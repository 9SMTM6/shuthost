//! Authentication for the coordinator: optional Token or OIDC based login.
//!
//! - Token mode: static bearer token, generated if not provided. Expects a cookie set via the built-in /login form.
//! - OIDC mode: standard authorization code flow with PKCE. Maintains a signed
//!   session cookie once the user is authenticated.

mod oidc;
mod token;

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
use cookie::SameSite;
use cookie::time::Duration as CookieDuration;
use rand::{Rng as _, distr::Alphanumeric};
use serde::{Deserialize, Serialize};
use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tracing::info;

use crate::{auth::token::LoginQuery, config::{AuthConfig, AuthMode, ControllerConfig}};
use crate::http::AppState;

const COOKIE_SESSION: &str = "shuthost_session";
const COOKIE_TOKEN: &str = "shuthost_token";
const COOKIE_STATE: &str = "shuthost_oidc_state";
const COOKIE_NONCE: &str = "shuthost_oidc_nonce";
const COOKIE_PKCE: &str = "shuthost_oidc_pkce";
const COOKIE_RETURN_TO: &str = "shuthost_return_to";

// Centralized login error keys used as query values on /login?error=<key>
pub const LOGIN_ERROR_INSECURE: &str = "insecure";
pub const LOGIN_ERROR_UNKNOWN: &str = "unknown";
pub const LOGIN_ERROR_TOKEN: &str = "token";
pub const LOGIN_ERROR_OIDC: &str = "oidc";

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
    },
    /// External auth (reverse proxy / external provider) acknowledged by operator.
    External {
        exceptions_version: u32,
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
                // If a token was configured in the TOML config, don't log its value
                // (it is already present in the config file). Only log the token
                // value when we auto-generate one on startup so operators can copy it.
                let token = if let Some(cfg_token) = token.clone() {
                    info!("Auth mode: token (configured)");
                    cfg_token
                } else {
                    let generated = generate_token();
                    info!("Auth mode: token");
                    info!("Token: {}", generated);
                    generated
                };

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
                    },
                    key_from_secret(cookie_secret.as_deref()),
                )
            }
            AuthConfig {
                mode: AuthMode::External { exceptions_version },
                ref cookie_secret,
            } => {
                info!("Auth mode: external (reverse proxy)");
                (
                    AuthResolved::External { exceptions_version },
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

pub const EXPECTED_EXCEPTIONS_VERSION: u32 = 1;

pub fn public_routes() -> Router<AppState> {
    use crate::routes::{get_download_router, m2m_routes};

    Router::new()
        // Auth endpoints
        .route("/login", get(login_get).post(token::login_post))
        .route("/logout", post(logout))
        .route("/oidc/login", get(oidc::oidc_login))
        .route("/oidc/callback", get(oidc::oidc_callback))
        // PWA & static assets bundled via asset_routes
        .merge(crate::assets::asset_routes())
        // Bypass routes
        .nest("/download", get_download_router())
        .nest("/api/m2m", m2m_routes())
}

pub async fn login_get(
    State(AppState { auth, .. }): State<AppState>,
    headers: axum::http::HeaderMap,
    axum::extract::Query(LoginQuery { error }): axum::extract::Query<LoginQuery>,
) -> impl IntoResponse {
    // Check if already authenticated
    type A = AuthResolved;

    let signed = SignedCookieJar::from_headers(&headers, auth.cookie_key.clone());
    let is_authenticated = match auth.mode {
        A::Token { ref token } => signed.get(COOKIE_TOKEN).is_some_and(|c| c.value() == token),
        A::Oidc { .. } => signed
            .get(COOKIE_SESSION)
            .and_then(|session| serde_json::from_str::<SessionClaims>(session.value()).ok())
            .is_some_and(|session| !session.is_expired()),
        A::Disabled
        | A::External {
            exceptions_version: EXPECTED_EXCEPTIONS_VERSION,
        } => true,
        _ => false,
    };
    if is_authenticated {
        return Redirect::to("/").into_response();
    }

    let maybe_error = match error.as_deref() {
        Some(v) if v == LOGIN_ERROR_INSECURE => {
            include_str!("../../assets/partials/login_error_insecure.tmpl.html")
        }
        Some(v) if v == LOGIN_ERROR_TOKEN => {
            include_str!("../../assets/partials/login_error_token.tmpl.html")
        }
        Some(v) if v == LOGIN_ERROR_UNKNOWN => {
            include_str!("../../assets/partials/login_error_unknown.tmpl.html")
        }
        Some(v) if v == LOGIN_ERROR_OIDC => {
            include_str!("../../assets/partials/login_error_oidc.tmpl.html")
        }
        Some(_) => include_str!("../../assets/partials/login_error_unknown.tmpl.html"),
        None => "",
    };

    let login_form = match auth.mode {
        A::Token { .. } => include_str!("../../assets/partials/token_login.tmpl.html"),
        A::Oidc { .. } => include_str!("../../assets/partials/oidc_login.tmpl.html"),
        _ => "",
    };

    let header_tpl = include_str!("../../assets/partials/header.tmpl.html");
    let footer = include_str!("../../assets/partials/footer.tmpl.html");
    let header = header_tpl
        .replace("{ maybe_tabs }", "")
        .replace("{ maybe_logout }", "")
        .replace("{ maybe_demo_disclaimer }", "");
    let html = include_str!("../../assets/login.tmpl.html")
        .replace(
            "{ html_head }",
            include_str!("../../assets/partials/html_head.tmlp.html"),
        )
        .replace("{ title }", "Login • ShutHost")
        .replace("{ maybe_error }", maybe_error)
        .replace("{ header }", &header)
        .replace("{ footer }", footer)
        .replace("{ login_form }", login_form)
        .replace("{ version }", env!("CARGO_PKG_VERSION"));
    axum::response::Response::builder()
        .header("Content-Type", "text/html")
        .body(axum::body::Body::from(html))
        .unwrap()
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
        AuthResolved::External { .. } => {
            // External auth (reverse proxy or external provider) is handled
            // outside the app; do not enforce internal auth here and let
            // requests through. The UI will show a prominent notice when
            // external auth is not acknowledged or has mismatched version.
            next.run(req).await
        }
        AuthResolved::Token { ref token } => {
            // Token auth uses a signed cookie. Read via SignedCookieJar instead
            // of parsing raw headers to ensure the signature is validated.
            let signed = SignedCookieJar::from_headers(headers, auth.cookie_key.clone());
            let cookie_ok = signed
                .get(COOKIE_TOKEN)
                .map(|c| c.value() == token)
                .unwrap_or(false);
            tracing::debug!(cookie_ok, "require_auth: token cookie check");
            if cookie_ok {
                next.run(req).await
            } else if wants_html(headers) {
                // remember path for redirect-after-login
                let return_to = req.uri().to_string();
                tracing::info!(return_to = %return_to, "require_auth: no token, redirecting to /login and setting return_to cookie");
                let jar = signed.add(
                    Cookie::build((COOKIE_RETURN_TO, return_to))
                        .http_only(true)
                        .secure(true)
                        .same_site(SameSite::Strict)
                        .max_age(CookieDuration::minutes(10))
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

/// Determine whether the incoming request should be considered secure.
/// First considers whether the server was started with TLS enabled. If so,
/// all requests are treated as secure. Otherwise falls back to the common
/// proxy headers: X-Forwarded-Proto, Forwarded and X-Forwarded-SSL.
pub fn request_is_secure(headers: &HeaderMap, tls_enabled: bool) -> bool {
    if tls_enabled {
        return true;
    }
    if let Some(p) = headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        && p.eq_ignore_ascii_case("https")
    {
        return true;
    }
    if let Some(fwd) = headers.get("forwarded").and_then(|v| v.to_str().ok())
        && fwd.to_lowercase().contains("proto=https")
    {
        return true;
    }
    if let Some(x) = headers.get("x-forwarded-ssl").and_then(|v| v.to_str().ok())
        && x.eq_ignore_ascii_case("on")
    {
        return true;
    }
    false
}

async fn logout(jar: SignedCookieJar, headers: HeaderMap) -> impl IntoResponse {
    // Log what cookies we saw when logout was invoked so we can ensure the path is hit
    let had_session = jar.get(COOKIE_SESSION).is_some();
    let had_token = jar.get(COOKIE_TOKEN).is_some();
    tracing::info!(had_session, had_token, "logout: received request");

    // Basic origin/referrer check to avoid cross-site logout triggers. If the
    // request includes Origin or Referer, ensure it matches the Host or
    // X-Forwarded-Host header. If it doesn't match, reject the request.
    if let Some(orig) = headers.get("origin").or_else(|| headers.get("referer"))
        && let Ok(orig_s) = orig.to_str()
        && let Some(host_hdr) = headers
            .get("x-forwarded-host")
            .or_else(|| headers.get("host"))
        && let Ok(host_s) = host_hdr.to_str()
        && !orig_s.contains(host_s)
    {
        tracing::warn!(origin = %orig_s, host = %host_s, "logout: origin/referrer mismatch");
        return StatusCode::BAD_REQUEST.into_response();
    }

    let jar = jar
        .remove(Cookie::build(COOKIE_TOKEN).path("/").build())
        .remove(Cookie::build(COOKIE_SESSION).path("/").build());

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
