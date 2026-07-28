//! Authentication middleware and security utilities.
//!
//! # Stability Warning
//!
//! The M2M path through this middleware (HMAC headers on `/api/*`) is a
//! **fallback** for machine-to-machine access to frontend UI endpoints.
//! Unlike the stable `/api/m2m/*` endpoints, these endpoints carry
//! **no stability guarantees** — they are the frontend's own backend and
//! can change at any time.  See [`super::hmac`] for details.

use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, Request, StatusCode, header},
    middleware::Next,
    response::{IntoResponse as _, Redirect, Response},
};
use axum_extra::extract::cookie::SignedCookieJar;

use crate::{
    app::AppState,
    http::auth::{
        AuthInfo, LOGIN_ERROR_SESSION_EXPIRED, Resolved,
        cookies::{
            create_return_to_cookie, get_oidc_session_from_cookie, get_token_session_from_cookie,
        },
        hmac, login_error_redirect,
    },
};

/// Middleware that enforces authentication depending on configured mode.
///
/// Accepts either cookie sessions (existing) or HMAC-SHA256 headers (new).
/// The same private routes serve both browser and M2M clients.
pub(crate) async fn require(
    State(state): State<AppState>,
    mut req: Request<Body>,
    next: Next,
) -> Response {
    let auth = &state.auth;

    // External auth (reverse proxy or external provider) is handled
    // outside the app; do not enforce internal auth here and let
    // requests through. The UI will show a prominent notice when
    // external auth is not acknowledged or has mismatched version.
    match auth.mode {
        Resolved::Disabled | Resolved::External { .. } => return next.run(req).await,
        _ => {}
    }

    // ── M2M HMAC authentication (unified path) ──────────────────────────
    if req.headers().contains_key("X-Client-ID") || req.headers().contains_key("X-Request") {
        // Clone the Arc<ControllerConfig> synchronously to avoid holding
        // the !Send RefGuard across any await point.
        let config = state.config_rx.borrow().clone();
        let client_id = match hmac::validate_hmac_identity(req.headers(), &config) {
            Ok(id) => id,
            Err((status, msg)) => return (status, msg).into_response(),
        };

        let path = req.uri().path();
        if hmac::M2M_BLOCKED_PREFIXES
            .iter()
            .any(|prefix| path.starts_with(prefix))
        {
            tracing::info!("Blocked M2M request to {path} from client '{client_id}'");
            return (
                StatusCode::FORBIDDEN,
                "Endpoint not available for M2M clients",
            )
                .into_response();
        }

        tracing::info!(
            client_id,
            version = %hmac::EXPECTED_FRONTEND_ENDPOINT_VERSION,
            path,
            "M2M frontend API request",
        );
        req.extensions_mut()
            .insert(AuthInfo::M2MClient { client_id });
        return next.run(req).await;
    }

    // ── Cookie-based authentication ─────────────────────────────────────
    let jar = SignedCookieJar::from_headers(req.headers(), auth.cookie_key.clone());
    match auth.mode {
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
                    req.extensions_mut().insert(AuthInfo::WebSession);
                    return next.run(req).await;
                }
            }
            if wants_html(req.headers()) {
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
                    req.extensions_mut().insert(AuthInfo::WebSession);
                    next.run(req).await
                };
            }
            tracing::info!("require: no valid session cookie, redirecting to /login");
            if wants_html(req.headers()) {
                redirect_with_return_to(jar, &req, Redirect::temporary("/login"))
            } else {
                StatusCode::UNAUTHORIZED.into_response()
            }
        }
        Resolved::Disabled | Resolved::External { .. } => {
            unreachable!("handled by early return above")
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
    tracing::debug!(return_to = %return_to, "setting return_to cookie");
    let jar = jar.add(create_return_to_cookie(return_to));
    (jar, redirect).into_response()
}

/// Check if the request wants HTML content based on Accept header.
fn wants_html(headers: &HeaderMap) -> bool {
    headers
        .get(header::ACCEPT)
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
