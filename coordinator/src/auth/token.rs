use axum::extract::State;
use axum::{
    Form,
    response::{IntoResponse, Redirect},
};
use axum_extra::extract::cookie::SignedCookieJar;
use serde::Deserialize;

use crate::auth::cookies::create_token_session_cookie;
use crate::auth::routes::TokenSessionClaims;
use crate::auth::{AuthResolved, LOGIN_ERROR_INSECURE, LOGIN_ERROR_TOKEN};
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
    match &auth.mode {
        AuthResolved::Token {
            token: expected, ..
        } if &token == expected => {
            let claims = TokenSessionClaims::new(&expected);
            let cookie =
                create_token_session_cookie(&claims, cookie::time::Duration::seconds((claims.exp - claims.iat) as i64));
            let jar = jar.add(cookie);
            (jar, Redirect::to("/")).into_response()
        }
        // Wrong token: redirect back to login with an error flag
        _ => Redirect::to(&format!("/login?error={}", LOGIN_ERROR_TOKEN)).into_response(),
    }
}
