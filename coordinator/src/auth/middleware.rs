//! Authentication middleware and security utilities.

use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::cookie::SignedCookieJar;

use crate::auth::{
    LOGIN_ERROR_SESSION_EXPIRED, LayerState, Resolved,
    cookies::{
        create_return_to_cookie, get_oidc_session_from_cookie, get_token_session_from_cookie,
    },
    login_error_redirect,
};

/// Middleware that enforces authentication depending on configured mode.
pub(crate) async fn require(
    State(LayerState { auth }): State<LayerState>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let headers = req.headers();
    let jar = SignedCookieJar::from_headers(headers, auth.cookie_key.clone());
    match auth.mode {
        // External auth (reverse proxy or external provider) is handled
        // outside the app; do not enforce internal auth here and let
        // requests through. The UI will show a prominent notice when
        // external auth is not acknowledged or has mismatched version.
        Resolved::Disabled | Resolved::External { .. } => next.run(req).await,
        Resolved::Token { ref token } => {
            // Token auth uses a signed cookie with claims (iat, exp, token_hash)
            if let Some(claims) = get_token_session_from_cookie(&jar) {
                if claims.is_expired() {
                    tracing::info!("require: token session expired, redirecting to login");
                    return redirect_with_return_to(
                        jar,
                        &req,
                        login_error_redirect(LOGIN_ERROR_SESSION_EXPIRED),
                    );
                }
                if claims.matches_token(token) {
                    return next.run(req).await;
                }
            }
            if wants_html(headers) {
                // remember path for redirect-after-login
                redirect_with_return_to(jar, &req, Redirect::temporary("/login"))
            } else {
                StatusCode::UNAUTHORIZED.into_response()
            }
        }
        Resolved::Oidc { .. } => {
            // Check signed session cookie via headers
            if let Some(sess) = get_oidc_session_from_cookie(&jar) {
                return if sess.is_expired() {
                    tracing::info!("require: OIDC session expired, redirecting to login");
                    redirect_with_return_to(
                        jar,
                        &req,
                        login_error_redirect(LOGIN_ERROR_SESSION_EXPIRED),
                    )
                } else {
                    next.run(req).await
                };
            }
            tracing::info!("require: no valid session cookie, redirecting to /login");
            if wants_html(headers) {
                redirect_with_return_to(jar, &req, Redirect::temporary("/login"))
            } else {
                StatusCode::UNAUTHORIZED.into_response()
            }
        }
    }
}

/// Helper function to redirect with `return_to` cookie set.
fn redirect_with_return_to(
    jar: SignedCookieJar,
    req: &Request<Body>,
    redirect: Redirect,
) -> Response {
    let return_to = req.uri().to_string();
    tracing::info!(return_to = %return_to, "setting return_to cookie");
    let jar = jar.add(create_return_to_cookie(return_to));
    (jar, redirect).into_response()
}

/// Check if the request wants HTML content based on Accept header.
fn wants_html(headers: &HeaderMap) -> bool {
    headers
        .get(axum::http::header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|s| s.contains("text/html"))
}

/// Determine whether the incoming request should be considered secure.
/// First considers whether the server was started with TLS enabled. If so,
/// all requests are treated as secure. Otherwise falls back to the common
/// proxy headers: X-Forwarded-Proto, Forwarded and X-Forwarded-SSL.
pub(crate) fn request_is_secure(headers: &HeaderMap, tls_enabled: bool) -> bool {
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
