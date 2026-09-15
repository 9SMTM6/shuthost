//! Integration tests for auth scoping with token auth enabled.
//!
//! These guard against regressions in how the auth middleware is layered
//! onto the router: public routes must stay public, the stable `/api/m2m/*`
//! endpoints must keep working with action-matched HMAC, and the unified
//! frontend-API HMAC path must accept `frontendpointV1` signatures.

use reqwest::{Client, StatusCode, header, redirect};
use secrecy::SecretString;
use shuthost_common::create_signed_message;

use crate::common::{KillOnDrop, get_free_port, spawn_coordinator_with_config, wait_for_listening};

const TOKEN: &str = "testtoken123";
const CLIENT_ID: &str = "test-client";
const CLIENT_SECRET: &str = "clientsecret";
const HOST: &str = "testhost";

/// Spawn a coordinator with token auth, one (agent-less) host, and one m2m client.
async fn spawn_token_auth_coordinator() -> (u16, KillOnDrop) {
    let port = get_free_port();
    let host_port = get_free_port();
    let child = spawn_coordinator_with_config(
        port,
        &format!(
            r#"
        [server]
        port = {port}
        bind = "127.0.0.1"

        [server.auth.token]
        token = "{TOKEN}"

        [hosts."{HOST}"]
        ip = "127.0.0.1"
        mac = "disableWOL"
        port = {host_port}
        shared_secret = "hostsecret"

        [clients."{CLIENT_ID}"]
        shared_secret = "{CLIENT_SECRET}"
        "#
        ),
    );
    wait_for_listening(port, 10).await;
    (port, child)
}

fn no_redirect_client() -> Client {
    Client::builder()
        .redirect(redirect::Policy::none())
        .build()
        .unwrap()
}

fn signed(client_msg: &str) -> String {
    create_signed_message(client_msg, &SecretString::from(CLIENT_SECRET))
}

#[tokio::test]
async fn public_routes_remain_public_under_token_auth() {
    let (port, _child) = spawn_token_auth_coordinator().await;
    let client = no_redirect_client();

    // The login page must be served directly — a miss-scoped auth middleware
    // turns this into a redirect loop (browser) or a 401 (API client).
    let resp = client
        .get(format!("http://127.0.0.1:{port}/login"))
        .header(header::ACCEPT, "text/html")
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "GET /login should serve the login page without a session"
    );

    // Static assets must be reachable without a session — the login page
    // itself needs them to render.
    let resp = client
        .get(format!("http://127.0.0.1:{port}/favicon.svg"))
        .send()
        .await
        .unwrap();
    assert!(
        resp.status().is_redirection(),
        "static assets must stay public, got {}",
        resp.status()
    );

    // The private API must still be protected.
    let resp = client
        .get(format!("http://127.0.0.1:{port}/api/hosts_status"))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "private API must still require auth"
    );
}

#[tokio::test]
async fn m2m_paths_work_under_token_auth() {
    let (port, _child) = spawn_token_auth_coordinator().await;
    let client = no_redirect_client();

    // Stable m2m status endpoint: the signed message is the action ("status").
    // A miss-scoped unified-auth middleware rejects this with a 403 version error.
    let resp = client
        .get(format!("http://127.0.0.1:{port}/api/m2m/status/{HOST}"))
        .header("X-Client-ID", CLIENT_ID)
        .header("X-Request", signed("status"))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "stable /api/m2m/* must keep working with token auth enabled"
    );

    // Stable m2m lease endpoint (async, so no agent is needed).
    let resp = client
        .post(format!(
            "http://127.0.0.1:{port}/api/m2m/lease/{HOST}/take?async=true"
        ))
        .header("X-Client-ID", CLIENT_ID)
        .header("X-Request", signed("take"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Unified frontend-API path: the signed message is the frontend-endpoint version.
    let resp = client
        .post(format!("http://127.0.0.1:{port}/api/lease/{HOST}/take"))
        .header("X-Client-ID", CLIENT_ID)
        .header("X-Request", signed("frontendpointV1"))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "unified HMAC path should accept the frontend-endpoint version"
    );

    // Blocklisted paths reject m2m clients even with valid credentials.
    let resp = client
        .get(format!("http://127.0.0.1:{port}/ws"))
        .header("X-Client-ID", CLIENT_ID)
        .header("X-Request", signed("frontendpointV1"))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "/ws must be blocked for m2m clients"
    );
}
