use std::time::{SystemTime, UNIX_EPOCH};

use axum::{
    extract::State,
    http::HeaderMap,
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::cookie::{Cookie, SignedCookieJar};
use cookie::time::Duration as CookieDuration;
use eyre::{Result, eyre};
use openidconnect::{
    AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointMaybeSet, EndpointNotSet,
    EndpointSet, IssuerUrl, Nonce, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope,
    core::{
        CoreAuthenticationFlow, CoreClient, CoreIdToken, CoreProviderMetadata, CoreTokenResponse,
    },
};
use reqwest::redirect::Policy;
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;

use crate::{
    auth::{
        self, COOKIE_NONCE, COOKIE_OIDC_SESSION, COOKIE_PKCE, COOKIE_STATE, LOGIN_ERROR_INSECURE,
        LOGIN_ERROR_OIDC, LOGIN_ERROR_SESSION_EXPIRED, OIDCSessionClaims,
        cookies::{
            create_oidc_session_cookie, create_protected_cookie,
            extract_return_to_and_remove_cookie,
        },
        login_error_redirect, request_is_secure,
    },
    http::AppState,
};

// Fixed redirect path used by the application for OIDC callbacks
const OIDC_CALLBACK_PATH: &str = "/oidc/callback";

// Compute request origin from headers
fn request_origin(headers: &HeaderMap) -> Option<String> {
    let host = headers
        .get("x-forwarded-host")
        .or_else(|| headers.get("host"))?
        .to_str()
        .ok()?;
    let proto = headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("http");
    Some(format!("{proto}://{host}"))
}

fn build_redirect_url(headers: &HeaderMap) -> Result<RedirectUrl> {
    let origin = request_origin(headers).ok_or_else(|| eyre!("missing Host header"))?;
    Ok(RedirectUrl::new(format!(
        "{}/{}",
        origin.trim_end_matches('/'),
        OIDC_CALLBACK_PATH.trim_start_matches('/'),
    ))?)
}

// Ready-to-use OIDC client type with the endpoints we require set
type OidcClientReady = CoreClient<
    EndpointSet,      // HasAuthUrl
    EndpointNotSet,   // HasDeviceAuthUrl
    EndpointNotSet,   // HasIntrospectionUrl (OIDC discovery does not provide this)
    EndpointNotSet,   // HasRevocationUrl (OIDC discovery does not provide this)
    EndpointSet,      // HasTokenUrl
    EndpointMaybeSet, // HasUserInfoUrl (from discovery, optional)
>;

async fn build_client(
    issuer: &str,
    client_id: &str,
    client_secret: &SecretString,
    headers: &HeaderMap,
) -> Result<(OidcClientReady, reqwest::Client), axum::http::StatusCode> {
    // HTTP client for discovery and token exchange
    let http = reqwest::Client::builder()
        .redirect(Policy::limited(3))
        .build()
        .map_err(|e| {
            tracing::error!("failed to build HTTP client: {e}");
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Discover provider
    let issuer = IssuerUrl::new(issuer.to_string()).map_err(|e| {
        tracing::error!("invalid issuer URL: {e}");
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let provider_metadata = CoreProviderMetadata::discover_async(issuer, &http)
        .await
        .map_err(|e| {
            tracing::error!("OIDC discovery failed: {e}");
            axum::http::StatusCode::BAD_GATEWAY
        })?;

    // Construct client and set required endpoints
    let client = CoreClient::from_provider_metadata(
        provider_metadata.clone(),
        ClientId::new(client_id.to_string()),
        Some(ClientSecret::new(client_secret.expose_secret().to_string())),
    )
    .set_auth_uri(provider_metadata.authorization_endpoint().clone());
    let client = if let Some(token_url) = provider_metadata.token_endpoint().cloned() {
        client.set_token_uri(token_url)
    } else {
        tracing::error!("OIDC provider missing token endpoint");
        return Err(axum::http::StatusCode::BAD_GATEWAY);
    };

    let client = match build_redirect_url(headers) {
        Ok(u) => {
            tracing::debug!(redirect_uri = %u.as_str(), "OIDC redirect URI computed");
            client.set_redirect_uri(u)
        }
        Err(e) => {
            tracing::error!("invalid redirect URL: {e}");
            return Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    Ok((client, http))
}

/// Initiate OIDC login.
#[axum::debug_handler]
pub(crate) async fn login(
    State(AppState {
        auth, tls_enabled, ..
    }): State<AppState>,
    jar: SignedCookieJar,
    headers: HeaderMap,
) -> impl IntoResponse {
    let auth::Resolved::Oidc {
        ref issuer,
        ref client_id,
        ref client_secret,
        ref scopes,
    } = auth.mode
    else {
        return Redirect::to("/").into_response();
    };
    // Refuse to start OIDC flow if request doesn't appear secure, because we
    // rely on Secure cookies for the OIDC state/nonce/pkce exchange.
    if !request_is_secure(&headers, tls_enabled) {
        tracing::warn!("oidc_login: insecure connection detected; refusing to set OIDC cookies");
        return login_error_redirect(LOGIN_ERROR_INSECURE).into_response();
    }
    // If already logged in, redirect to return_to or home
    let had_session = jar.get(COOKIE_OIDC_SESSION).is_some();
    tracing::debug!(had_session, "oidc_login: called");
    if had_session {
        let (return_to, jar) = extract_return_to_and_remove_cookie(jar);
        tracing::info!(return_to = %return_to, "oidc_login: existing session, redirecting to return_to");
        return (jar, Redirect::to(&return_to)).into_response();
    }
    let (client, _http) = match build_client(issuer, client_id, client_secret, &headers).await {
        Ok(ok) => ok,
        Err(sc) => return sc.into_response(),
    };

    tracing::info!(issuer = %issuer, "Initiating OIDC login");

    let (pkce_challenge, verifier) = PkceCodeChallenge::new_random_sha256();
    let mut authorize = client.authorize_url(
        CoreAuthenticationFlow::AuthorizationCode,
        CsrfToken::new_random,
        Nonce::new_random,
    );
    for s in scopes {
        authorize = authorize.add_scope(Scope::new(s.clone()));
    }
    let (auth_url, csrf_token, nonce) = authorize.set_pkce_challenge(pkce_challenge).url();

    // Store state + nonce + pkce in signed cookies and clear logged_out flag so it applies only to
    // the next attempt
    tracing::debug!(state = %csrf_token.secret(), nonce = %nonce.secret(), pkce_len = verifier.secret().len(), "oidc_login: storing state/nonce/pkce in cookies");
    // Short-lived cookies for OIDC state to mitigate replay attacks
    let short_exp = CookieDuration::minutes(10);
    let jar = jar
        .add(create_protected_cookie(
            COOKIE_STATE,
            csrf_token.secret().clone(),
            short_exp,
        ))
        .add(create_protected_cookie(
            COOKIE_NONCE,
            nonce.secret().clone(),
            short_exp,
        ))
        .add(create_protected_cookie(
            COOKIE_PKCE,
            verifier.secret().clone(),
            short_exp,
        ));

    tracing::info!(auth_url = %auth_url, "oidc_login: redirecting to provider authorization endpoint");
    (jar, Redirect::to(auth_url.as_str())).into_response()
}

#[derive(Deserialize)]
/// Query parameters for OIDC callback deserialization.
pub(crate) struct CallbackQueryParams {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Debug, Clone, Copy)]
enum LoginFlowError {
    /// Redirect to login with a generic OIDC error message
    LoginRedirect,
    /// Return a `StatusCode` (expected to be in the 4XX range)
    Status(axum::http::StatusCode),
}

fn login_error_response() -> Response {
    login_error_redirect(LOGIN_ERROR_OIDC).into_response()
}

fn clear_oidc_ephemeral_cookies(jar: SignedCookieJar) -> SignedCookieJar {
    jar.remove(Cookie::build(COOKIE_STATE).path("/").build())
        .remove(Cookie::build(COOKIE_NONCE).path("/").build())
        .remove(Cookie::build(COOKIE_PKCE).path("/").build())
}

/// Verify state (present in query params and matches cookies)
fn validate_state_or_redirect(
    jar: &SignedCookieJar,
    state_param: Option<&String>,
) -> Option<Response> {
    let Some(state_cookie) = jar.get(COOKIE_STATE) else {
        tracing::warn!("OIDC callback missing state cookie");
        return Some(login_error_response());
    };
    let Some(state_param) = state_param else {
        tracing::warn!("OIDC callback missing state param");
        return Some(login_error_response());
    };
    if state_cookie.value() != state_param {
        tracing::warn!("OIDC callback state mismatch");
        return Some(login_error_response());
    }
    None
}

fn pkce_from_cookie(jar: &SignedCookieJar) -> Option<PkceCodeVerifier> {
    jar.get(COOKIE_PKCE)
        .map(|c| PkceCodeVerifier::new(c.value().to_string()))
}

fn nonce_from_cookie(jar: &SignedCookieJar) -> Option<Nonce> {
    jar.get(COOKIE_NONCE)
        .map(|c| Nonce::new(c.value().to_string()))
}

fn finalize_session_and_redirect(jar: SignedCookieJar, session: &OIDCSessionClaims) -> Response {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time is before the UNIX epoch")
        .as_secs();
    let session_exp_seconds = session.exp.saturating_sub(now);
    let session_max_age = CookieDuration::seconds(
        session_exp_seconds
            .try_into()
            .expect("session expiration is impossibly high"),
    )
    .min(CookieDuration::days(7));
    let jar =
        clear_oidc_ephemeral_cookies(jar).add(create_oidc_session_cookie(session, session_max_age));

    let (return_to, jar) = extract_return_to_and_remove_cookie(jar);
    (jar, Redirect::to(&return_to)).into_response()
}

/// If provider returned an error, bounce back to login with message
fn handle_provider_error(
    error: Option<String>,
    error_description: Option<&String>,
    jar: SignedCookieJar,
) -> Option<Response> {
    if let Some(err) = error {
        tracing::warn!("OIDC error from provider: {} {:?}", err, error_description);
        let jar = clear_oidc_ephemeral_cookies(jar);
        return Some((jar, login_error_response()).into_response());
    }
    None
}

fn extract_authorization_code(code: Option<String>) -> Result<String, LoginFlowError> {
    match code {
        Some(c) => Ok(c),
        None => {
            tracing::warn!("OIDC callback missing code");
            Err(LoginFlowError::LoginRedirect)
        }
    }
}

async fn exchange_code_for_token(
    client: &OidcClientReady,
    http: &reqwest::Client,
    code: String,
    pkce_verifier: Option<PkceCodeVerifier>,
) -> Result<CoreTokenResponse, LoginFlowError> {
    let mut req = client.exchange_code(AuthorizationCode::new(code));
    if let Some(v) = pkce_verifier {
        req = req.set_pkce_verifier(v);
    }
    match req.request_async(http).await {
        Ok(r) => Ok(r),
        Err(e) => {
            tracing::error!("Token exchange failed: {:#?}", e);
            Err(LoginFlowError::Status(axum::http::StatusCode::BAD_GATEWAY))
        }
    }
}

fn id_token_from_response(
    token_response: &CoreTokenResponse,
) -> Result<CoreIdToken, LoginFlowError> {
    match token_response.extra_fields().id_token() {
        Some(id) => Ok(id.clone()),
        None => {
            tracing::warn!("No id_token in response; refusing login");
            Err(LoginFlowError::Status(axum::http::StatusCode::BAD_REQUEST))
        }
    }
}

fn verify_id_token_and_build_session(
    client: &OidcClientReady,
    id_token: &CoreIdToken,
    nonce_cookie: Option<&Nonce>,
) -> Result<OIDCSessionClaims, LoginFlowError> {
    let claims = match id_token.claims(
        &client.id_token_verifier(),
        nonce_cookie.unwrap_or(&Nonce::new(String::new())),
    ) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Invalid id token: {}", e);
            return Err(LoginFlowError::Status(axum::http::StatusCode::UNAUTHORIZED));
        }
    };
    let sub = claims.subject().to_string();
    let exp = claims
        .expiration()
        .timestamp()
        .try_into()
        .expect("time should not move backwards");
    Ok(OIDCSessionClaims { sub, exp })
}

/// Exchange code, verify `id_token` and build session
async fn process_token_and_build_session(
    client: &OidcClientReady,
    http: &reqwest::Client,
    jar: &SignedCookieJar,
    code: Option<String>,
) -> Result<OIDCSessionClaims, LoginFlowError> {
    let code = extract_authorization_code(code)?;
    tracing::debug!(
        code_len = code.len(),
        "Authorization code received (length)"
    );
    let pkce_verifier = pkce_from_cookie(jar);
    tracing::debug!(
        pkce_present = pkce_verifier.is_some(),
        "PKCE verifier present in cookie"
    );
    let token_response = exchange_code_for_token(client, http, code, pkce_verifier).await?;
    let id_token = id_token_from_response(&token_response)?;
    let nonce_cookie = nonce_from_cookie(jar);
    verify_id_token_and_build_session(client, &id_token, nonce_cookie.as_ref())
}

/// OIDC callback handler
#[axum::debug_handler]
pub(crate) async fn callback(
    State(AppState { auth, .. }): State<AppState>,
    jar: SignedCookieJar,
    headers: HeaderMap,
    axum::extract::Query(CallbackQueryParams {
        code,
        state,
        error,
        error_description,
    }): axum::extract::Query<CallbackQueryParams>,
) -> impl IntoResponse {
    let crate::auth::Resolved::Oidc {
        ref issuer,
        ref client_id,
        ref client_secret,
        scopes: _,
    } = auth.mode
    else {
        return Redirect::to("/").into_response();
    };

    if let Some(resp) = validate_state_or_redirect(&jar, state.as_ref()) {
        return resp;
    }

    if let Some(resp) = handle_provider_error(error, error_description.as_ref(), jar.clone()) {
        return resp;
    }

    let (client, http) = match build_client(issuer, client_id, client_secret, &headers).await {
        Ok(ok) => ok,
        Err(sc) => return sc.into_response(),
    };

    // Log useful debug info to diagnose token exchange issues
    if let Ok(u) = build_redirect_url(&headers) {
        tracing::debug!(redirect_uri = %u.as_str(), "OIDC callback computed redirect URI");
    }

    let session = match process_token_and_build_session(&client, &http, &jar, code).await {
        Ok(s) => {
            if s.is_expired() {
                return login_error_redirect(LOGIN_ERROR_SESSION_EXPIRED).into_response();
            }
            s
        }
        Err(LoginFlowError::LoginRedirect) => return login_error_response(),
        Err(LoginFlowError::Status(sc)) => return sc.into_response(),
    };

    finalize_session_and_redirect(jar, &session)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cookie::Key;

    #[test]
    fn test_validate_state_or_redirect_mismatch() {
        let key = Key::generate();
        let jar = SignedCookieJar::new(key);
        let jar = jar.add(Cookie::new(COOKIE_STATE, "different_state"));
        let state_param = Some("test_state".to_string());
        let result = validate_state_or_redirect(&jar, state_param.as_ref());
        assert!(result.is_some());
    }

    #[test]
    fn test_handle_provider_error_with_error() {
        let key = Key::generate();
        let jar = SignedCookieJar::new(key);
        let error = Some("access_denied".to_string());
        let error_description = Some("User denied access".to_string());
        let result = handle_provider_error(error, error_description.as_ref(), jar);
        assert!(result.is_some());
    }

    #[test]
    fn test_handle_provider_error_no_error() {
        let key = Key::generate();
        let jar = SignedCookieJar::new(key);
        let error = None;
        let error_description = None;
        let result = handle_provider_error(error, error_description.as_ref(), jar);
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_authorization_code_missing() {
        let code = None;
        let result = extract_authorization_code(code);
        assert!(result.is_err());
        match result.unwrap_err() {
            LoginFlowError::LoginRedirect => {}
            _ => panic!("Expected LoginRedirect"),
        }
    }
}
