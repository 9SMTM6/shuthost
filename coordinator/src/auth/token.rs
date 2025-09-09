use axum::{Form, response::{IntoResponse, Redirect}};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use axum::extract::State;
use serde::Deserialize;

use crate::http::AppState;
use crate::auth::{AuthResolved, COOKIE_TOKEN, COOKIE_RETURN_TO};

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
    match auth.mode {
        AuthResolved::Token { ref token } => {
            // If already authenticated via cookie, go home
            let cookie_ok = super::get_cookie(&headers, COOKIE_TOKEN)
                .map(|v| v == *token)
                .unwrap_or(false);
            if cookie_ok {
                return Redirect::to("/").into_response();
            }

            let maybe_error = if error.is_some() {
                include_str!("../../assets/partials/login_error.tmpl.html")
            } else {
                ""
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
                .replace("{ version }", env!("CARGO_PKG_VERSION"));
            axum::response::Response::builder()
                .header("Content-Type", "text/html")
                .body(axum::body::Body::from(html))
                .unwrap()
        }
        AuthResolved::Oidc { .. } => {
            // If already logged in via OIDC session, go home
            let signed = axum_extra::extract::cookie::SignedCookieJar::from_headers(&headers, auth.cookie_key.clone());
            if let Some(session) = signed.get(super::COOKIE_SESSION)
                && let Ok(sess) = serde_json::from_str::<super::SessionClaims>(session.value())
                && !sess.is_expired()
            {
                return Redirect::to("/").into_response();
            }
            Redirect::temporary("/oidc/login").into_response()
        }
        AuthResolved::Disabled => Redirect::temporary("/").into_response(),
    }
}

pub async fn login_post(
    State(AppState { auth, .. }): State<AppState>,
    jar: CookieJar,
    Form(LoginForm { token }): Form<LoginForm>,
) -> impl IntoResponse {
    match auth.mode {
        AuthResolved::Token { token: ref expected } if &token == expected => {
            let cookie = Cookie::build((COOKIE_TOKEN, token))
                .http_only(true)
                .path("/")
                .build();
            let jar = jar.add(cookie);
            // Try redirect back to original path
            let return_to = jar
                .get(COOKIE_RETURN_TO)
                .map(|c| c.value().to_string())
                .unwrap_or_else(|| "/".to_string());
            let jar = jar.remove(Cookie::build(COOKIE_RETURN_TO).path("/").build());
            (jar, Redirect::to(&return_to)).into_response()
        }
        // Wrong token: redirect back to login with an error flag
        _ => Redirect::to("/login?error=1").into_response(),
    }
}
