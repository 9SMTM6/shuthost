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
    app::AppState,
    http::auth::{
        Resolved,
        cookies::{self, get_oidc_session_from_cookie, get_token_session_from_cookie},
        oidc,
        token,
    },
    http::assets::{UiMode, render_ui_html},
};

/// Returns a router with all authentication-related routes.
pub(crate) fn routes() -> Router<AppState> {
    Router::new()
        .route("/login", get(page).post(token::login_post))
        .route("/logout", post(logout))
        .route("/oidc/login", get(oidc::login))
        .route("/oidc/callback", get(oidc::callback))
}

/// Handle GET requests to the login page. Redirects if already authenticated;
/// otherwise serves the SPA shell — SolidJS Router renders `/login` client-side.
#[axum::debug_handler]
pub(crate) async fn page(
    State(AppState { auth, config_path, .. }): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
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

    let auth_mode = auth.mode.auth_mode_str();

    (
        TypedHeader(ContentType::html()),
        render_ui_html(&UiMode::Normal {
            config_path: &config_path,
            show_logout: false,
            auth_warning: false,
            auth_mode,
        }),
    )
        .into_response()
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
