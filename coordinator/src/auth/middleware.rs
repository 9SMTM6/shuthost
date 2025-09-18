//! Authentication middleware and security utilities.

use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::cookie::{Cookie, SignedCookieJar};

use crate::auth::cookies::{
    COOKIE_RETURN_TO, COOKIE_SESSION, COOKIE_TOKEN, create_return_to_cookie,
};
use crate::auth::{AuthLayerState, AuthResolved, SessionClaims};

/// Middleware that enforces authentication depending on configured mode.
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
                let jar = signed.add(create_return_to_cookie(return_to));
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

/// Check if the request wants HTML content based on Accept header.
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
