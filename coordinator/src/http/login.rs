//! Authentication route handlers.

use axum::{
    Router,
    extract::State,
    http::HeaderMap,
    response::{IntoResponse, Redirect},
    routing::{get, post},
};
use axum_extra::{TypedHeader, extract::cookie::SignedCookieJar, headers::ContentType};
use reqwest::StatusCode;

use crate::{
    auth::{
        LOGIN_ERROR_INSECURE, LOGIN_ERROR_OIDC, LOGIN_ERROR_SESSION_EXPIRED, LOGIN_ERROR_TOKEN,
        LOGIN_ERROR_UNKNOWN, Resolved,
        cookies::{self, get_oidc_session_from_cookie, get_token_session_from_cookie},
        oidc,
        token::{self, LoginQuery},
    },
    http::AppState,
    include_utf8_asset,
};

/// Returns a router with all authentication-related routes.
pub(crate) fn routes() -> Router<AppState> {
    Router::new()
        .route("/login", get(page).post(token::login_post))
        .route("/logout", post(logout))
        .route("/oidc/login", get(oidc::login))
        .route("/oidc/callback", get(oidc::callback))
}

/// Handle GET requests to the login page.
#[axum::debug_handler]
pub(crate) async fn page(
    State(AppState { auth, .. }): State<AppState>,
    headers: axum::http::HeaderMap,
    axum::extract::Query(LoginQuery { error }): axum::extract::Query<LoginQuery>,
) -> impl IntoResponse {
    // Check if already authenticated
    type A = Resolved;

    let jar = SignedCookieJar::from_headers(&headers, auth.cookie_key.clone());
    let is_authenticated = match auth.mode {
        A::Token { ref token } => get_token_session_from_cookie(&jar)
            .is_some_and(|session| !session.is_expired() && session.matches_token(token)),
        A::Oidc { .. } => {
            get_oidc_session_from_cookie(&jar).is_some_and(|session| !session.is_expired())
        }
        A::Disabled | A::External { .. } => true,
    };
    if is_authenticated {
        return Redirect::to("/").into_response();
    }

    let maybe_error = match error.as_deref() {
        Some(v) if v == LOGIN_ERROR_INSECURE => {
            include_utf8_asset!("partials/login_error_insecure.html")
        }
        Some(v) if v == LOGIN_ERROR_TOKEN => {
            include_utf8_asset!("partials/login_error_token.html")
        }
        Some(v) if v == LOGIN_ERROR_UNKNOWN => {
            include_utf8_asset!("partials/login_error_unknown.html")
        }
        Some(v) if v == LOGIN_ERROR_OIDC => {
            include_utf8_asset!("partials/login_error_oidc.html")
        }
        Some(v) if v == LOGIN_ERROR_SESSION_EXPIRED => {
            include_utf8_asset!("partials/login_error_session_expired.html")
        }
        Some(_) => include_utf8_asset!("partials/login_error_unknown.html"),
        None => "",
    };

    let login_form = match auth.mode {
        A::Token { .. } => include_utf8_asset!("partials/token_login.html"),
        A::Oidc { .. } => include_utf8_asset!("partials/oidc_login.html"),
        _ => "",
    };

    let html = include_utf8_asset!("generated/login.html")
        .replace("{ maybe_error }", maybe_error)
        .replace("{ login_form }", login_form);
    (TypedHeader(ContentType::html()), html).into_response()
}

/// Handle logout requests.
#[axum::debug_handler]
pub(crate) async fn logout(
    _: State<AppState>,
    jar: SignedCookieJar,
    headers: HeaderMap,
) -> impl IntoResponse {
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

    let jar = cookies::invalidate_session(jar);
    (jar, Redirect::to("/login")).into_response()
}
