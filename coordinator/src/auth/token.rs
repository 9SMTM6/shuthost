use axum::extract::State;
use axum::{
    Form,
    response::{IntoResponse, Redirect},
};
use axum_extra::extract::cookie::{Cookie, SignedCookieJar};
use serde::Deserialize;

use crate::auth::cookies::create_token_cookie;
use crate::auth::{AuthResolved, COOKIE_RETURN_TO, LOGIN_ERROR_INSECURE, LOGIN_ERROR_TOKEN};
use crate::http::AppState;

#[derive(Deserialize)]
pub(super) struct LoginForm {
    token: String,
}

#[derive(Deserialize, Default)]
pub struct LoginQuery {
    pub error: Option<String>,
}

pub async fn login_post(
    State(AppState {
        auth, tls_enabled, ..
    }): State<AppState>,
    jar: SignedCookieJar,
    headers: axum::http::HeaderMap,
    Form(LoginForm { token }): Form<LoginForm>,
) -> impl IntoResponse {
    // If the connection doesn't look secure, surface an error instead of setting Secure cookies
    if !crate::auth::request_is_secure(&headers, tls_enabled) {
        tracing::warn!(
            "login_post: insecure connection detected; refusing to set Secure auth cookie"
        );
        return Redirect::to(&format!("/login?error={}", LOGIN_ERROR_INSECURE)).into_response();
    }
    match auth.mode {
        AuthResolved::Token {
            token: ref expected,
            ..
        } if &token == expected => {
            // Persistent token cookie: mark Secure, HttpOnly and SameSite=strict
            // so it cannot be leaked via JS and is protected from CSRF. Use a
            // reasonable expiry for long-lived bearer tokens.
            let cookie = create_token_cookie(&token);
            let jar = jar.add(cookie);
            // Try redirect back to original path (read signed return_to cookie)
            let return_to = jar
                .get(COOKIE_RETURN_TO)
                .map(|c| c.value().to_string())
                .unwrap_or_else(|| "/".to_string());
            let jar = jar.remove(Cookie::build(COOKIE_RETURN_TO).path("/").build());
            (jar, Redirect::to(&return_to)).into_response()
        }
        // Wrong token: redirect back to login with an error flag
        _ => Redirect::to(&format!("/login?error={}", LOGIN_ERROR_TOKEN)).into_response(),
    }
}
