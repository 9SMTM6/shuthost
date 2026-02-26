use axum::{
    Form,
    extract::State,
    http,
    response::{IntoResponse, Redirect},
};
use axum_extra::extract::cookie::SignedCookieJar;
use cookie::time::Duration as CookieDuration;
use secrecy::{ExposeSecret as _, SecretString};
use serde::Deserialize;

use crate::{
    app::state::AppState,
    http::auth::{
        LOGIN_ERROR_INSECURE, LOGIN_ERROR_TOKEN, Resolved,
        cookies::{
            TokenSessionClaims, create_token_session_cookie, extract_return_to_and_remove_cookie,
        },
        login_error_redirect, request_is_secure,
    },
};

#[derive(Deserialize)]
pub(crate) struct LoginForm {
    token: SecretString,
}

#[derive(Deserialize, Default)]
pub(crate) struct LoginQuery {
    pub error: Option<String>,
}

#[axum::debug_handler]
pub(crate) async fn login_post(
    State(AppState {
        auth, tls_enabled, ..
    }): State<AppState>,
    jar: SignedCookieJar,
    headers: http::HeaderMap,
    Form(LoginForm { token }): Form<LoginForm>,
) -> impl IntoResponse {
    // If the connection doesn't look secure, surface an error instead of setting Secure cookies
    if !request_is_secure(&headers, tls_enabled) {
        tracing::warn!(
            "login_post: insecure connection detected; refusing to set Secure auth cookie"
        );
        return login_error_redirect(LOGIN_ERROR_INSECURE).into_response();
    }
    match &auth.mode {
        &Resolved::Token {
            token: ref expected,
            ..
        } if token.expose_secret() == expected.expose_secret() => {
            let claims = TokenSessionClaims::new((*expected).expose_secret());
            let cookie = create_token_session_cookie(
                &claims,
                CookieDuration::seconds(
                    (claims.exp - claims.iat)
                        .try_into()
                        .expect("session expiration is impossibly high"),
                ),
            );
            let jar = jar.add(cookie);
            let (return_to, jar) = extract_return_to_and_remove_cookie(jar);
            (jar, Redirect::to(&return_to)).into_response()
        }
        // Wrong token: redirect back to login with an error flag
        _ => login_error_redirect(LOGIN_ERROR_TOKEN).into_response(),
    }
}
