use std::time::{SystemTime, UNIX_EPOCH};

use axum::{
    extract::{self, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::cookie::{Cookie, SignedCookieJar};
use cookie::time::Duration as CookieDuration;
use eyre::{Result, WrapErr as _, eyre};
use oauth2_reqwest::ReqwestClient;
use openidconnect::{
    AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointMaybeSet, EndpointNotSet,
    EndpointSet, IssuerUrl, Nonce, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope,
    core::{
        CoreAuthenticationFlow, CoreClient, CoreIdToken, CoreProviderMetadata, CoreTokenResponse,
    },
};
use reqwest::redirect::Policy;
use secrecy::{ExposeSecret as _, SecretString};
use serde::Deserialize;

use crate::{
    config::OidcConfig,
    http::auth::{
        self, COOKIE_NONCE, COOKIE_OIDC_SESSION, COOKIE_PKCE, COOKIE_STATE, LOGIN_ERROR_INSECURE,
        LOGIN_ERROR_OIDC, LOGIN_ERROR_SESSION_EXPIRED, OIDCSessionClaims, SharedOidcClient,
        cookies::{
            create_oidc_session_cookie, create_protected_cookie,
            extract_return_to_and_remove_cookie,
        },
        login_error_redirect, request_is_secure,
    },
    state::AppState,
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
pub(crate) type OidcClientReady = CoreClient<
    EndpointSet,      // HasAuthUrl
    EndpointNotSet,   // HasDeviceAuthUrl
    EndpointNotSet,   // HasIntrospectionUrl (OIDC discovery does not provide this)
    EndpointNotSet,   // HasRevocationUrl (OIDC discovery does not provide this)
    EndpointSet,      // HasTokenUrl
    EndpointMaybeSet, // HasUserInfoUrl (from discovery, optional)
>;

fn set_redirect_uri(
    client: &OidcClientReady,
    headers: &HeaderMap,
) -> Result<OidcClientReady, StatusCode> {
    match build_redirect_url(headers) {
        Ok(u) => {
            tracing::debug!(redirect_uri = %u.as_str(), "OIDC redirect URI computed");
            Ok(client.clone().set_redirect_uri(u))
        }
        Err(e) => {
            tracing::error!(%e, "invalid redirect URL");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub(crate) async fn build_client(
    issuer: &str,
    client_id: &str,
    client_secret: &SecretString,
) -> eyre::Result<OidcClientReady> {
    let http = reqwest::Client::builder()
        .redirect(Policy::limited(3))
        .danger_accept_invalid_certs({
            // Allow disabling TLS verification for discovery at compile time by
            // defining OIDC_DANGER_ACCEPT_INVALID_CERTS at compile time. Example:
            //
            // OIDC_DANGER_ACCEPT_INVALID_CERTS=1 cargo build -p shuthost_coordinator
            let enabled = option_env!("OIDC_DANGER_ACCEPT_INVALID_CERTS").is_some();
            if enabled {
                tracing::warn!(
                    "OIDC discovery: accepting invalid TLS certificates (compile-time cfg enabled)"
                );
            }
            enabled
        })
        .build()
        .wrap_err("failed to build HTTP client")?;

    // Discover provider
    let issuer = IssuerUrl::new(issuer.to_string()).wrap_err("invalid issuer URL")?;
    let provider_metadata =
        CoreProviderMetadata::discover_async(issuer, &ReqwestClient::from(http))
            .await
            .wrap_err("OIDC discovery failed")?;

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
        return Err(eyre!("OIDC provider missing token endpoint"));
    };

    Ok(client)
}

/// Refresh the client stored in `shared` using the original configuration.
///
/// Currently we don’t have a unit test for this flow; an integration or
/// component test should exercise both the initial client build and a
/// subsequent refresh when discovery/JWKS data changes.
/// TODO: add tests targeting `refresh_oidc_client`
pub(crate) async fn refresh_oidc_client(
    shared: &SharedOidcClient,
    cfg: &OidcConfig,
) -> eyre::Result<()> {
    tracing::info!(issuer=%cfg.issuer, "refreshing OIDC client from config");
    let new_client = build_client(&cfg.issuer, &cfg.client_id, &cfg.client_secret)
        .await
        .wrap_err("Failed to build refreshed OIDC client")?;
    let mut guard = shared.write().await;
    *guard = new_client;
    Ok(())
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
        ref client,
        ref config,
    } = auth.mode
    else {
        return Redirect::to("/").into_response();
    };
    let scopes = &config.scopes;
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
    // Grab a clone of the current client for URL computation
    let client = {
        let guard = client.read().await;
        guard.clone()
    };
    let client = match set_redirect_uri(&client, &headers) {
        Ok(c) => c,
        Err(sc) => return sc.into_response(),
    };

    tracing::info!("Initiating OIDC login");

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
    Status(StatusCode),
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
        tracing::warn!(%err, "OIDC error from provider: {error_description}", error_description = error_description.map_or("No Description", String::as_str));
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
    client_lock: &SharedOidcClient,
    code: String,
    pkce_verifier: Option<PkceCodeVerifier>,
) -> Result<CoreTokenResponse, LoginFlowError> {
    let client = {
        let guard = client_lock.read().await;
        guard.clone()
    };

    let http = reqwest::Client::builder()
        .redirect(Policy::none())
        .build()
        .map_err(|e| {
            tracing::error!(%e, "failed to build HTTP client");
            LoginFlowError::Status(StatusCode::INTERNAL_SERVER_ERROR)
        })?;
    let mut req = client.exchange_code(AuthorizationCode::new(code));
    if let Some(v) = pkce_verifier {
        req = req.set_pkce_verifier(v);
    }
    match req.request_async(&ReqwestClient::from(http)).await {
        Ok(r) => Ok(r),
        Err(e) => {
            tracing::error!(%e, "Token exchange failed");
            Err(LoginFlowError::Status(StatusCode::BAD_GATEWAY))
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
            Err(LoginFlowError::Status(StatusCode::BAD_REQUEST))
        }
    }
}

async fn verify_id_token_and_build_session(
    client_lock: &SharedOidcClient,
    cfg: &OidcConfig,
    id_token: &CoreIdToken,
    nonce_cookie: Option<&Nonce>,
) -> Result<OIDCSessionClaims, LoginFlowError> {
    fn do_verify(
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
                tracing::error!(%e, "Invalid id token");
                return Err(LoginFlowError::Status(StatusCode::UNAUTHORIZED));
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

    // first attempt using the current client
    {
        let client = client_lock.read().await;
        if let Ok(claims) = do_verify(&client, id_token, nonce_cookie) {
            return Ok(claims);
        }
    }

    // verification failed; try refreshing the client once
    tracing::info!("verification failed, refreshing OIDC client");
    if let Err(e) = refresh_oidc_client(client_lock, cfg).await {
        tracing::error!(%e, "failed to refresh OIDC client");
        return Err(LoginFlowError::Status(StatusCode::UNAUTHORIZED));
    }

    // retry with refreshed client
    let client = client_lock.read().await;
    do_verify(&client, id_token, nonce_cookie)
}

/// Exchange code, verify `id_token` and build session
async fn process_token_and_build_session(
    client_lock: &SharedOidcClient,
    cfg: &OidcConfig,
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
    let token_response = exchange_code_for_token(client_lock, code, pkce_verifier).await?;
    let id_token = id_token_from_response(&token_response)?;
    let nonce_cookie = nonce_from_cookie(jar);
    verify_id_token_and_build_session(client_lock, cfg, &id_token, nonce_cookie.as_ref()).await
}

/// OIDC callback handler
#[axum::debug_handler]
pub(crate) async fn callback(
    State(AppState { auth, .. }): State<AppState>,
    jar: SignedCookieJar,
    headers: HeaderMap,
    extract::Query(CallbackQueryParams {
        code,
        state,
        error,
        error_description,
    }): extract::Query<CallbackQueryParams>,
) -> impl IntoResponse {
    let auth::Resolved::Oidc {
        ref client,
        ref config,
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

    // Need a temporary owned copy for redirect URI computation;
    // keep the shared lock for later token exchange.
    let client_for_redirect = {
        let guard = client.read().await;
        guard.clone()
    };
    let client_for_redirect = match set_redirect_uri(&client_for_redirect, &headers) {
        Ok(c) => c,
        Err(sc) => return sc.into_response(),
    };

    // Log useful debug info to diagnose token exchange issues
    tracing::debug!(redirect_uri = %client_for_redirect.redirect_uri().expect("Should be set now").as_str(), "OIDC callback computed redirect URI");

    let session = match process_token_and_build_session(client, config, &jar, code).await {
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
    fn validate_state_or_redirect_mismatch() {
        let key = Key::generate();
        let jar = SignedCookieJar::new(key);
        let jar = jar.add(Cookie::new(COOKIE_STATE, "different_state"));
        let state_param = Some("test_state".to_string());
        let result = validate_state_or_redirect(&jar, state_param.as_ref());
        assert!(result.is_some());
    }

    #[test]
    fn handle_provider_error_with_error() {
        let key = Key::generate();
        let jar = SignedCookieJar::new(key);
        let error = Some("access_denied".to_string());
        let error_description = Some("User denied access".to_string());
        let result = handle_provider_error(error, error_description.as_ref(), jar);
        assert!(result.is_some());
    }

    #[test]
    fn handle_provider_error_no_error() {
        let key = Key::generate();
        let jar = SignedCookieJar::new(key);
        let error = None;
        let error_description = None;
        let result = handle_provider_error(error, error_description.as_ref(), jar);
        assert!(result.is_none());
    }

    #[test]
    fn extract_authorization_code_missing() {
        let code = None;
        let result = extract_authorization_code(code);
        assert!(result.is_err());
        match result.unwrap_err() {
            LoginFlowError::LoginRedirect => {}
            LoginFlowError::Status(_) => panic!("Expected LoginRedirect"),
        }
    }
}
