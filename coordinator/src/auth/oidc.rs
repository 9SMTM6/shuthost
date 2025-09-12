use crate::auth::{
    COOKIE_LOGGED_OUT, COOKIE_NONCE, COOKIE_PKCE, COOKIE_RETURN_TO, COOKIE_SESSION, COOKIE_STATE,
    SessionClaims,
};
use crate::http::AppState;
use axum::http::HeaderMap;
use axum::{
    extract::State,
    response::{IntoResponse, Redirect},
};
use axum_extra::extract::cookie::{Cookie, SignedCookieJar};
use openidconnect::core::{CoreAuthenticationFlow, CoreClient, CoreProviderMetadata};
use openidconnect::{
    AuthorizationCode, ClientId, ClientSecret, CsrfToken, IssuerUrl, Nonce, PkceCodeChallenge,
    PkceCodeVerifier, RedirectUrl, Scope,
};
use openidconnect::{EndpointMaybeSet, EndpointNotSet, EndpointSet};
use reqwest::redirect::Policy;
use serde::Deserialize;

// Fixed redirect path used by the application for OIDC callbacks
const OIDC_CALLBACK_PATH: &str = "/oidc/callback";

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
    Some(format!("{}://{}", proto, host))
}

fn build_redirect_url(
    headers: &axum::http::HeaderMap,
) -> Result<RedirectUrl, anyhow::Error> {
    let origin = request_origin(headers).ok_or_else(|| anyhow::anyhow!("missing Host header"))?;
    Ok(RedirectUrl::new(format!(
        "{}/{}",
        origin.trim_end_matches('/'),
        OIDC_CALLBACK_PATH.trim_start_matches('/')
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

async fn build_oidc_client(
    issuer: &str,
    client_id: &str,
    client_secret: &str,
    headers: &HeaderMap,
) -> Result<(OidcClientReady, reqwest::Client), axum::http::StatusCode> {
    // Build HTTP client (no redirects per SSRF guidance)
    // Allow a small number of redirects for discovery only (some providers redirect .well-known)
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
        Some(ClientSecret::new(client_secret.to_string())),
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
pub async fn oidc_login(
    State(AppState { auth, .. }): State<AppState>,
    jar: SignedCookieJar,
    headers: HeaderMap,
) -> impl IntoResponse {
    let crate::auth::AuthResolved::Oidc {
        ref issuer,
        ref client_id,
        ref client_secret,
        ref scopes,
    } = auth.mode
    else {
        return Redirect::to("/").into_response();
    };
    // If already logged in, redirect to return_to or home
    let had_session = jar.get(COOKIE_SESSION).is_some();
    tracing::debug!(had_session, "oidc_login: called");
    if let Some(session) = jar.get(COOKIE_SESSION)
        && let Ok(sess) = serde_json::from_str::<SessionClaims>(session.value())
        && !sess.is_expired()
    {
        let return_to = jar
            .get(COOKIE_RETURN_TO)
            .map(|c| c.value().to_string())
            .unwrap_or_else(|| "/".to_string());
        let jar = jar.remove(Cookie::build(COOKIE_RETURN_TO).path("/").build());
        tracing::info!(return_to = %return_to, "oidc_login: existing valid session, redirecting to return_to");
        return (jar, Redirect::to(&return_to)).into_response();
    }
    let (client, _http) =
        match build_oidc_client(issuer, client_id, client_secret, &headers).await {
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
    // If the user just logged out, force interactive login at the IdP once
    let logged_out_flag = jar.get(COOKIE_LOGGED_OUT).is_some();
    tracing::debug!(logged_out_flag, "oidc_login: logged_out cookie present");
    if logged_out_flag {
        tracing::info!(
            "oidc_login: adding prompt=login and max_age=0 to authorization request to force interactive login"
        );
        // Some OPs ignore prompt=login; adding max_age=0 asks the provider to re-authenticate.
        authorize = authorize.add_extra_param("prompt", "login");
        authorize = authorize.add_extra_param("max_age", "0");
    }
    let (auth_url, csrf_token, nonce) = authorize.set_pkce_challenge(pkce_challenge).url();

    // Store state + nonce + pkce in signed cookies and clear logged_out flag so it applies only to
    // the next attempt
    tracing::debug!(state = %csrf_token.secret(), nonce = %nonce.secret(), pkce_len = verifier.secret().len(), "oidc_login: storing state/nonce/pkce in cookies");
    let signed = jar
        .add(
            Cookie::build((COOKIE_STATE, csrf_token.secret().clone()))
                .path("/")
                .build(),
        )
        .add(
            Cookie::build((COOKIE_NONCE, nonce.secret().clone()))
                .path("/")
                .build(),
        )
        .add(
            Cookie::build((COOKIE_PKCE, verifier.secret().clone()))
                .path("/")
                .build(),
        )
        // Clear the flag so it applies only to the next attempt
        .remove(Cookie::build(COOKIE_LOGGED_OUT).path("/").build());

    tracing::info!(auth_url = %auth_url, "oidc_login: redirecting to provider authorization endpoint");
    (signed, Redirect::to(auth_url.as_str())).into_response()
}

#[derive(Deserialize)]
pub(super) struct OidcCallback {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

/// OIDC callback handler
pub async fn oidc_callback(
    State(AppState { auth, .. }): State<AppState>,
    jar: SignedCookieJar,
    headers: HeaderMap,
    axum::extract::Query(OidcCallback {
        code,
        state,
        error,
        error_description,
    }): axum::extract::Query<OidcCallback>,
) -> impl IntoResponse {
    let crate::auth::AuthResolved::Oidc {
        ref issuer,
        ref client_id,
        ref client_secret,
    scopes: _,
    } = auth.mode
    else {
        return Redirect::to("/").into_response();
    };
    let login_error = Redirect::to("/login?error=1").into_response();
    let signed = jar;
    // Verify state (present and matches)
    let Some(state_cookie) = signed.get(COOKIE_STATE) else {
        tracing::warn!("OIDC callback missing state cookie");
        return login_error;
    };
    let Some(state_param) = state.as_deref() else {
        tracing::warn!("OIDC callback missing state param");
        return login_error;
    };
    if state_cookie.value() != state_param {
        tracing::warn!("OIDC callback state mismatch");
        return login_error;
    }

    // If provider returned an error, bounce back to login with message
    if let Some(err) = error {
        tracing::warn!("OIDC error from provider: {} {:?}", err, error_description);
        let signed = signed
            .remove(Cookie::build(COOKIE_STATE).path("/").build())
            .remove(Cookie::build(COOKIE_NONCE).path("/").build())
            .remove(Cookie::build(COOKIE_PKCE).path("/").build());
        return (signed, login_error).into_response();
    }

    let (client, http) =
        match build_oidc_client(issuer, client_id, client_secret, &headers).await {
            Ok(ok) => ok,
            Err(sc) => return sc.into_response(),
        };

    // Log useful debug info to diagnose token exchange issues
    if let Ok(u) = build_redirect_url(&headers) {
        tracing::debug!(redirect_uri = %u.as_str(), "OIDC callback computed redirect URI");
    }

    // PKCE verifier
    let pkce_verifier = signed
        .get(COOKIE_PKCE)
        .map(|c| PkceCodeVerifier::new(c.value().to_string()));
    tracing::debug!(
        pkce_present = pkce_verifier.is_some(),
        "PKCE verifier present in cookie"
    );

    let Some(code) = code else {
        tracing::warn!("OIDC callback missing code");
        return login_error;
    };
    tracing::debug!(
        code_len = code.len(),
        "Authorization code received (length)"
    );
    let mut req = client.exchange_code(AuthorizationCode::new(code));
    if let Some(v) = pkce_verifier {
        req = req.set_pkce_verifier(v);
    }

    let token_response = match req.request_async(&http).await {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("Token exchange failed: {:#?}", e);
            return axum::http::StatusCode::BAD_GATEWAY.into_response();
        }
    };

    // ID token optional but recommended
    let id_token = match token_response.extra_fields().id_token() {
        Some(id) => id.clone(),
        None => {
            tracing::warn!("No id_token in response; refusing login");
            return axum::http::StatusCode::BAD_REQUEST.into_response();
        }
    };

    // Verify nonce
    let nonce_cookie = signed
        .get(COOKIE_NONCE)
        .map(|c| Nonce::new(c.value().to_string()));
    let claims = match id_token.claims(
        &client.id_token_verifier(),
        nonce_cookie.as_ref().unwrap_or(&Nonce::new(String::new())),
    ) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Invalid id token: {}", e);
            return axum::http::StatusCode::UNAUTHORIZED.into_response();
        }
    };

    let sub = claims.subject().to_string();
    let exp = claims.expiration().timestamp() as u64;
    let session = SessionClaims { sub, exp };

    let signed = signed
        .remove(Cookie::build(COOKIE_STATE).path("/").build())
        .remove(Cookie::build(COOKIE_NONCE).path("/").build())
        .remove(Cookie::build(COOKIE_PKCE).path("/").build())
        .add(
            Cookie::build((COOKIE_SESSION, serde_json::to_string(&session).unwrap()))
                .http_only(true)
                .path("/")
                .build(),
        );

    // Redirect back if present
    let return_to = signed
        .get(COOKIE_RETURN_TO)
        .map(|c| c.value().to_string())
        .unwrap_or_else(|| "/".to_string());
    let signed = signed.remove(Cookie::build(COOKIE_RETURN_TO).path("/").build());
    (signed, Redirect::to(&return_to)).into_response()
}
