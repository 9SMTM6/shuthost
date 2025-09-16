use reqwest::Client;

mod common;
use common::{get_free_port, KillOnDrop, spawn_coordinator_with_config, wait_for_listening};

#[tokio::test]
async fn token_login_flow() {
    let port = get_free_port();
    let token = "testtoken123";
    let config = format!(
        r#"
        [server]
        port = {port}
        bind = "127.0.0.1"

    [server.auth]
    type = "token"
    token = "{token}"

    [server.tls]

    [hosts]

    [clients]
        "#
    );
    let child = spawn_coordinator_with_config(port, &config);
    let _guard = KillOnDrop(child);
    wait_for_listening(port, 20).await;

    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap();

    // POST to /login with the correct token
    let url = format!("https://127.0.0.1:{port}/login");
    let resp = client
        .post(&url)
        .form(&[("token", token)])
        .send()
        .await
        .expect("failed to post login");

    // Expect redirect to / (302)
    assert!(resp.status().is_redirection());

    // Extract cookies from set-cookie headers
    let cookies: Vec<String> = resp
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok().map(|s| s.to_string()))
        .collect();
    assert!(!cookies.is_empty(), "no Set-Cookie headers present");

    // Now query a protected endpoint, forwarding cookies
    let cookies_header = cookies.join("; ");
    let protected = format!("https://127.0.0.1:{port}/api/hosts_status");
    let resp2 = client
        .get(&protected)
        .header(reqwest::header::COOKIE, cookies_header)
        .send()
        .await
        .expect("failed to GET protected");

    // Token auth should allow access (200)
    assert!(
        resp2.status().is_success(),
        "protected endpoint not accessible"
    );
}
