use axum::extract::State;
use axum::{
    Form,
    response::{IntoResponse, Redirect},
};
use axum_extra::extract::cookie::{Cookie, SignedCookieJar};
use cookie::SameSite;
use cookie::time::Duration as CookieDuration;
use serde::Deserialize;

use crate::auth::{
    AuthResolved, COOKIE_RETURN_TO, COOKIE_SESSION, COOKIE_TOKEN, LOGIN_ERROR_INSECURE,
    LOGIN_ERROR_OIDC, LOGIN_ERROR_TOKEN, LOGIN_ERROR_UNKNOWN,
};
use crate::http::AppState;

#[derive(Deserialize)]
pub(super) struct LoginForm {
    token: String,
}

#[derive(Deserialize, Default)]
pub struct LoginQuery {
    pub error: Option<String>,
}

pub async fn login_get(
    State(AppState { auth, .. }): State<AppState>,
    headers: axum::http::HeaderMap,
    axum::extract::Query(LoginQuery { error }): axum::extract::Query<LoginQuery>,
) -> impl IntoResponse {
    // Check if already authenticated
    type A = AuthResolved;

    let signed = SignedCookieJar::from_headers(&headers, auth.cookie_key.clone());
    let is_authenticated = match auth.mode {
        A::Token { ref token } => signed.get(COOKIE_TOKEN).is_some_and(|c| c.value() == token),
        A::Oidc { .. } => signed
            .get(COOKIE_SESSION)
            .and_then(|session| serde_json::from_str::<super::SessionClaims>(session.value()).ok())
            .is_some_and(|session| !session.is_expired()),
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
        } if &token == expected => {
            // Persistent token cookie: mark Secure, HttpOnly and SameSite=strict
            // so it cannot be leaked via JS and is protected from CSRF. Use a
            // reasonable expiry for long-lived bearer tokens.
            let cookie = Cookie::build((COOKIE_TOKEN, token))
                .http_only(true)
                .secure(true)
                .same_site(SameSite::Strict)
                .max_age(CookieDuration::days(30))
                .path("/")
                .build();
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
