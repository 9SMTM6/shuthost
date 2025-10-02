//! Authentication route handlers.

use axum::{
    Router,
    extract::State,
    http::HeaderMap,
    response::{IntoResponse, Redirect},
    routing::{get, post},
};
use axum_extra::extract::cookie::{Cookie, SignedCookieJar};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::auth::cookies::{
    COOKIE_OIDC_SESSION, COOKIE_TOKEN_SESSION, get_oidc_session_from_cookie,
    get_token_session_from_cookie,
};
use crate::auth::token::LoginQuery;
use crate::auth::{
    AuthResolved, LOGIN_ERROR_INSECURE, LOGIN_ERROR_OIDC, LOGIN_ERROR_SESSION_EXPIRED,
    LOGIN_ERROR_TOKEN, LOGIN_ERROR_UNKNOWN,
};
use crate::http::AppState;

pub const EXPECTED_EXCEPTIONS_VERSION: u32 = 1;

/// Public routes that don't require authentication.
pub fn public_routes() -> Router<AppState> {
    use crate::routes::{get_download_router, m2m_routes};

    Router::new()
        // Auth endpoints
        .route(
            "/login",
            get(login_get).post(crate::auth::token::login_post),
        )
        .route("/logout", post(logout))
        .route("/oidc/login", get(crate::auth::oidc::oidc_login))
        .route("/oidc/callback", get(crate::auth::oidc::oidc_callback))
        // PWA & static assets bundled via asset_routes
        .merge(crate::assets::asset_routes())
        // Bypass routes
        .nest("/download", get_download_router())
        .nest("/api/m2m", m2m_routes())
}

/// Handle GET requests to the login page.
pub async fn login_get(
    State(AppState { auth, .. }): State<AppState>,
    headers: axum::http::HeaderMap,
    axum::extract::Query(LoginQuery { error }): axum::extract::Query<LoginQuery>,
) -> impl IntoResponse {
    // Check if already authenticated
    type A = AuthResolved;

    let signed = SignedCookieJar::from_headers(&headers, auth.cookie_key.clone());
    let is_authenticated = match auth.mode {
        A::Token { ref token } => get_token_session_from_cookie(&signed)
            .is_some_and(|session| !session.is_expired() && session.matches_token(token)),
        A::Oidc { .. } => {
            get_oidc_session_from_cookie(&signed).is_some_and(|session| !session.is_expired())
        }
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
        Some(v) if v == LOGIN_ERROR_SESSION_EXPIRED => {
            include_str!("../../assets/partials/login_error_session_expired.tmpl.html")
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

/// Handle logout requests.
async fn logout(jar: SignedCookieJar, headers: HeaderMap) -> impl IntoResponse {
    // Log what cookies we saw when logout was invoked so we can ensure the path is hit
    let had_session_oidc = jar.get(COOKIE_OIDC_SESSION).is_some();
    let had_session_token = jar.get(COOKIE_TOKEN_SESSION).is_some();
    tracing::info!(
        had_session_oidc,
        had_session_token,
        "logout: received request"
    );

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
        return axum::http::StatusCode::BAD_REQUEST.into_response();
    }

    let jar = jar
        .remove(Cookie::build(COOKIE_TOKEN_SESSION).path("/").build())
        .remove(Cookie::build(COOKIE_OIDC_SESSION).path("/").build());

    tracing::info!("logout: removed session cookies");
    (jar, Redirect::to("/login")).into_response()
}

/// Session claims for token authentication.
#[derive(Serialize, Deserialize)]
pub struct TokenSessionClaims {
    pub iat: u64,           // issued at
    pub exp: u64,           // expiry
    pub token_hash: String, // hash of the token
}

impl TokenSessionClaims {
    pub fn new(token: &str) -> Self {
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
    pub fn is_expired(&self) -> bool {
        now_ts() >= self.exp
    }
    /// Check if the token matches (by hash).
    pub fn matches_token(&self, token: &str) -> bool {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes());
        let hash = format!("{:x}", hasher.finalize());
        self.token_hash == hash
    }
}

/// Session claims for OIDC authentication.
/// Contains some claims from the [OIDC Id Token](https://openid.net/specs/openid-connect-core-1_0.html#IDToken)
#[derive(Serialize, Deserialize)]
pub struct OIDCSessionClaims {
    /// The sub claim, a unique user identifier
    pub sub: String,
    /// The expiry as provided by the IdP, after which shuthost should reject the session. Unix second timestamp
    pub exp: u64,
}

impl OIDCSessionClaims {
    /// Check if the session has expired.
    pub fn is_expired(&self) -> bool {
        now_ts() >= self.exp
    }
}

/// Get the current timestamp in seconds since UNIX epoch.
pub fn now_ts() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
