use reqwest::{Client, header, redirect};

use crate::common::{KillOnDrop, get_free_port, spawn_coordinator_with_config, wait_for_listening};

/// Convenience: spawn a coordinator configured to use token auth.
pub(crate) fn spawn_coordinator_with_token(port: u16, token: &str) -> KillOnDrop {
    let config = format!(
        r#"
    [server]
    port = {port}
    bind = "127.0.0.1"

    [server.auth.token]
    token = "{token}"

    [hosts]

    [clients]
        "#
    );
    spawn_coordinator_with_config(port, &config)
}

#[tokio::test]
async fn insecure_post_redirects_with_insecure_error() {
    let port = get_free_port();
    let token = "correct-token";
    let _child = spawn_coordinator_with_token(port, token);
    wait_for_listening(port, 10).await;

    let client = Client::builder()
        .redirect(redirect::Policy::none())
        .build()
        .unwrap();

    let url = format!("http://127.0.0.1:{port}/login");
    // No x-forwarded-proto header -> considered insecure
    let resp = client
        .post(&url)
        .form(&[("token", token)])
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_redirection());
    let loc = resp
        .headers()
        .get(header::LOCATION)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(
        loc.contains("error=insecure"),
        "location did not contain insecure error: {loc}"
    );
}

#[tokio::test]
async fn invalid_token_redirects_with_token_error() {
    let port = get_free_port();
    let token = "correct-token";
    let _child = spawn_coordinator_with_token(port, token);
    wait_for_listening(port, 10).await;

    let client = Client::builder()
        .redirect(redirect::Policy::none())
        .build()
        .unwrap();

    let url = format!("http://127.0.0.1:{port}/login");
    // indicate secure via x-forwarded-proto to bypass insecure check
    let resp = client
        .post(&url)
        .header("x-forwarded-proto", "https")
        .form(&[("token", "bad-token")])
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_redirection());
    let loc = resp
        .headers()
        .get(header::LOCATION)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(
        loc.contains("error=token"),
        "location did not contain token error: {loc}"
    );
}
