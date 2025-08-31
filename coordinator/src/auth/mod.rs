//! Authentication for the coordinator: optional Token or OIDC based login.
//!
//! - Token mode: static bearer token, generated if not provided. Expects either
//!   Authorization: Bearer <token> or a cookie set via the built-in /login form.
//! - OIDC mode: standard authorization code flow with PKCE. Maintains a signed
//!   session cookie once the user is authenticated.

use axum::{
    Form, Router,
    body::Body,
    extract::{FromRef, Query, State},
    http::{HeaderMap, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
};
use axum_extra::extract::cookie::{Cookie, CookieJar, Key, SignedCookieJar};
use base64::Engine;
use openidconnect::core::{CoreAuthenticationFlow, CoreClient, CoreProviderMetadata};
use openidconnect::reqwest::async_http_client;
use openidconnect::{
    AuthorizationCode, ClientId, ClientSecret, CsrfToken, IssuerUrl, Nonce, PkceCodeChallenge,
    PkceCodeVerifier, RedirectUrl, Scope,
};
use rand::{Rng as _, distr::Alphanumeric};
use serde::{Deserialize, Serialize};
use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tracing::{error, info, warn};

use crate::config::{AuthConfig, AuthMode, ControllerConfig};
use crate::http::AppState;

const COOKIE_SESSION: &str = "shuthost_session";
const COOKIE_TOKEN: &str = "shuthost_token";
const COOKIE_STATE: &str = "shuthost_oidc_state";
const COOKIE_NONCE: &str = "shuthost_oidc_nonce";
const COOKIE_PKCE: &str = "shuthost_oidc_pkce";
const COOKIE_RETURN_TO: &str = "shuthost_return_to";

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
        .route("/login", get(login_get).post(login_post))
        .route("/logout", post(logout))
        .route("/auth/login", get(oidc_login))
        .route("/auth/callback", get(oidc_callback))
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
            // Accept Bearer token or cookie
            let bearer_ok = headers
                .get(axum::http::header::AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.strip_prefix("Bearer "))
                .map(|t| t == token)
                .unwrap_or(false);
            let cookie_ok = get_cookie(headers, COOKIE_TOKEN)
                .map(|v| v == *token)
                .unwrap_or(false);
            if bearer_ok || cookie_ok {
                next.run(req).await
            } else if wants_html(headers) {
                // remember path for redirect-after-login
                let return_to = req.uri().to_string();
                let cookie = Cookie::build((COOKIE_RETURN_TO, return_to))
                    .path("/")
                    .build();
                let mut resp = Redirect::temporary("/login").into_response();
                resp.headers_mut().append(
                    axum::http::header::SET_COOKIE,
                    axum::http::HeaderValue::from_str(&cookie.to_string()).unwrap(),
                );
                resp
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
            if wants_html(headers) {
                let return_to = req.uri().to_string();
                let cookie = Cookie::build((COOKIE_RETURN_TO, return_to))
                    .path("/")
                    .build();
                let mut resp = Redirect::temporary("/auth/login").into_response();
                resp.headers_mut().append(
                    axum::http::header::SET_COOKIE,
                    axum::http::HeaderValue::from_str(&cookie.to_string()).unwrap(),
                );
                resp
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
/// Minimal login page for Token mode (redirects if already logged in).
#[derive(Deserialize, Default)]
struct LoginQuery {
    error: Option<String>,
}

async fn login_get(
    State(AppState { auth, .. }): State<AppState>,
    headers: HeaderMap,
    Query(LoginQuery { error }): Query<LoginQuery>,
) -> impl IntoResponse {
    match auth.mode {
        AuthResolved::Token { ref token } => {
            // If already authenticated via cookie, go home
            let cookie_ok = get_cookie(&headers, COOKIE_TOKEN)
                .map(|v| v == *token)
                .unwrap_or(false);
            if cookie_ok {
                return Redirect::to("/").into_response();
            }

            let err_html = if error.is_some() {
                "<p style='color:#b00;margin:0 0 1rem'>Invalid token. Please try again.</p>"
            } else {
                ""
            };

            Response::builder()
                .header("Content-Type", "text/html")
                .body(format!(r#"<html><head><title>Login</title></head>
                    <body style='font-family:sans-serif'>
                    <h1>Login</h1>
                    {err_html}
                    <form method='post'>
                      <label>Access Token <input name='token' type='password' autofocus required /></label>
                      <button type='submit'>Login</button>
                    </form>
                    </body></html>"#).into())
                .unwrap()
        }
        AuthResolved::Oidc { .. } => {
            // If already logged in via OIDC session, go home
            let signed = SignedCookieJar::from_headers(&headers, auth.cookie_key.clone());
            if let Some(session) = signed.get(COOKIE_SESSION)
                && let Ok(sess) = serde_json::from_str::<SessionClaims>(session.value())
                && !sess.is_expired()
            {
                return Redirect::to("/").into_response();
            }
            Redirect::temporary("/auth/login").into_response()
        }
        AuthResolved::Disabled => Redirect::temporary("/").into_response(),
    }
}

#[derive(Deserialize)]
struct LoginForm {
    token: String,
}

async fn login_post(
    State(AppState { auth, .. }): State<AppState>,
    jar: CookieJar,
    Form(LoginForm { token }): Form<LoginForm>,
) -> impl IntoResponse {
    match auth.mode {
        AuthResolved::Token {
            token: ref expected,
        } if &token == expected => {
            let cookie = Cookie::build((COOKIE_TOKEN, token))
                .http_only(true)
                .path("/")
                .build();
            let jar = jar.add(cookie);
            // Try redirect back to original path
            let return_to = jar
                .get(COOKIE_RETURN_TO)
                .map(|c| c.value().to_string())
                .unwrap_or_else(|| "/".to_string());
            let jar = jar.remove(Cookie::build(COOKIE_RETURN_TO).path("/").build());
            (jar, Redirect::to(&return_to)).into_response()
        }
        // Wrong token: redirect back to login with an error flag
        _ => Redirect::to("/login?error=1").into_response(),
    }
}

async fn logout(jar: CookieJar) -> impl IntoResponse {
    let jar = jar
        .remove(Cookie::build(COOKIE_TOKEN).path("/").build())
        .remove(Cookie::build(COOKIE_SESSION).path("/").build());
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

/// Initiate OIDC login.
async fn oidc_login(
    State(AppState { auth, .. }): State<AppState>,
    jar: SignedCookieJar,
    headers: HeaderMap,
) -> impl IntoResponse {
    let AuthResolved::Oidc {
        ref issuer,
        ref client_id,
        ref client_secret,
        ref scopes,
        ref redirect_path,
    } = auth.mode
    else {
        return Redirect::to("/").into_response();
    };

    // If already logged in, redirect to return_to or home
    if let Some(session) = jar.get(COOKIE_SESSION)
        && let Ok(sess) = serde_json::from_str::<SessionClaims>(session.value())
        && !sess.is_expired()
    {
        let return_to = jar
            .get(COOKIE_RETURN_TO)
            .map(|c| c.value().to_string())
            .unwrap_or_else(|| "/".to_string());
        let jar = jar.remove(Cookie::build(COOKIE_RETURN_TO).path("/").build());
        return (jar, Redirect::to(&return_to)).into_response();
    }
    let Ok((client, _redirect_url)) =
        build_oidc_client(issuer, client_id, client_secret, redirect_path, &headers).await
    else {
        return (StatusCode::INTERNAL_SERVER_ERROR, "OIDC setup failed").into_response();
    };

    let (pkce_challenge, verifier) = PkceCodeChallenge::new_random_sha256();
    let mut authorize = client.authorize_url(
        CoreAuthenticationFlow::AuthorizationCode,
        CsrfToken::new_random,
        Nonce::new_random,
    );
    for s in scopes {
        authorize = authorize.add_scope(Scope::new(s.clone()));
    }
    let (auth_url, csrf_token, nonce) = authorize.set_pkce_challenge(pkce_challenge).url();

    // Store state + nonce in signed cookies
    let signed = jar
        .add(
            Cookie::build((COOKIE_STATE, csrf_token.secret().clone()))
                .path("/")
                .build(),
        )
        .add(
            Cookie::build((COOKIE_NONCE, nonce.secret().clone()))
                .path("/")
                .build(),
        )
        .add(
            Cookie::build((COOKIE_PKCE, verifier.secret().clone()))
                .path("/")
                .build(),
        );

    (signed, Redirect::to(auth_url.as_str())).into_response()
}

#[derive(Deserialize)]
struct OidcCallback {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

async fn oidc_callback(
    State(AppState { auth, .. }): State<AppState>,
    jar: SignedCookieJar,
    headers: HeaderMap,
    Query(OidcCallback {
        code,
        state,
        error,
        error_description,
    }): Query<OidcCallback>,
) -> impl IntoResponse {
    let AuthResolved::Oidc {
        ref issuer,
        ref client_id,
        ref client_secret,
        scopes: _,
        ref redirect_path,
    } = auth.mode
    else {
        return Redirect::to("/").into_response();
    };
    let signed = jar;
    // Verify state (present and matches)
    let Some(state_cookie) = signed.get(COOKIE_STATE) else {
        warn!("OIDC callback missing state cookie");
        return Redirect::to("/login?error=1").into_response();
    };
    let Some(state_param) = state.as_deref() else {
        warn!("OIDC callback missing state param");
        return Redirect::to("/login?error=1").into_response();
    };
    if state_cookie.value() != state_param {
        warn!("OIDC callback state mismatch");
        return Redirect::to("/login?error=1").into_response();
    }

    // If provider returned an error, bounce back to login with message
    if let Some(err) = error {
        warn!("OIDC error from provider: {} {:?}", err, error_description);
        let signed = signed
            .remove(Cookie::build(COOKIE_STATE).path("/").build())
            .remove(Cookie::build(COOKIE_NONCE).path("/").build())
            .remove(Cookie::build(COOKIE_PKCE).path("/").build());
        return (signed, Redirect::to("/login?error=1")).into_response();
    }

    let Ok((client, _redirect_url)) =
        build_oidc_client(issuer, client_id, client_secret, redirect_path, &headers).await
    else {
        return (StatusCode::INTERNAL_SERVER_ERROR, "OIDC setup failed").into_response();
    };

    // PKCE verifier
    let pkce_verifier = signed
        .get(COOKIE_PKCE)
        .map(|c| PkceCodeVerifier::new(c.value().to_string()));

    let Some(code) = code else {
        warn!("OIDC callback missing code");
        return Redirect::to("/login?error=1").into_response();
    };
    let mut req = client.exchange_code(AuthorizationCode::new(code));
    if let Some(v) = pkce_verifier {
        req = req.set_pkce_verifier(v);
    }

    let token_response = match req.request_async(async_http_client).await {
        Ok(r) => r,
        Err(e) => {
            error!("Token exchange failed: {}", e);
            return StatusCode::BAD_GATEWAY.into_response();
        }
    };

    // ID token optional but recommended
    let id_token = match token_response.extra_fields().id_token() {
        Some(id) => id.clone(),
        None => {
            warn!("No id_token in response; refusing login");
            return StatusCode::BAD_REQUEST.into_response();
        }
    };

    // Verify nonce
    let nonce_cookie = signed
        .get(COOKIE_NONCE)
        .map(|c| Nonce::new(c.value().to_string()));
    let claims = match id_token.claims(
        &client.id_token_verifier(),
        nonce_cookie.as_ref().unwrap_or(&Nonce::new(String::new())),
    ) {
        Ok(c) => c,
        Err(e) => {
            error!("Invalid id token: {}", e);
            return StatusCode::UNAUTHORIZED.into_response();
        }
    };

    let sub = claims.subject().to_string();
    let exp = claims.expiration().timestamp() as u64;
    let session = SessionClaims { sub, exp };

    let signed = signed
        .remove(Cookie::build(COOKIE_STATE).path("/").build())
        .remove(Cookie::build(COOKIE_NONCE).path("/").build())
        .remove(Cookie::build(COOKIE_PKCE).path("/").build())
        .add(
            Cookie::build((COOKIE_SESSION, serde_json::to_string(&session).unwrap()))
                .http_only(true)
                .path("/")
                .build(),
        );

    // Redirect back if present
    let return_to = signed
        .get(COOKIE_RETURN_TO)
        .map(|c| c.value().to_string())
        .unwrap_or_else(|| "/".to_string());
    let signed = signed.remove(Cookie::build(COOKIE_RETURN_TO).path("/").build());
    (signed, Redirect::to(&return_to)).into_response()
}

async fn build_oidc_client(
    issuer: &str,
    client_id: &str,
    client_secret: &str,
    redirect_path: &str,
    headers: &HeaderMap,
) -> Result<(CoreClient, RedirectUrl), anyhow::Error> {
    let issuer = IssuerUrl::new(issuer.to_string())?;
    let provider_metadata = CoreProviderMetadata::discover_async(issuer, async_http_client).await?;
    let client_id = ClientId::new(client_id.to_string());
    let client_secret = ClientSecret::new(client_secret.to_string());

    let origin = request_origin(headers).ok_or_else(|| anyhow::anyhow!("missing Host header"))?;
    let redirect_url = RedirectUrl::new(format!(
        "{}/{}",
        origin.trim_end_matches('/'),
        redirect_path.trim_start_matches('/')
    ))?;

    let client =
        CoreClient::from_provider_metadata(provider_metadata, client_id, Some(client_secret))
            .set_redirect_uri(redirect_url.clone());

    Ok((client, redirect_url))
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
