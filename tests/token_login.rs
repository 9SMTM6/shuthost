use reqwest::Client;
use std::fs;
use std::process::Command;
use std::time::Duration;

fn get_free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("failed to bind to address")
        .local_addr()
        .unwrap()
        .port()
}

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

    [hosts]

    [clients]
        "#
    );
    let tmp = std::env::temp_dir().join("integration_test_token.toml");
    fs::write(&tmp, config).expect("failed to write config");

    // Prefer to run the built binary directly to avoid invoking `cargo run` while
    // `cargo test` holds the build lock. Cargo provides an env var pointing to
    // the built binary; fall back to the default target path.
    let bin = std::env::var("CARGO_BIN_EXE_shuthost_coordinator").unwrap_or_else(|_| {
        std::env::current_dir()
            .unwrap()
            .join("target/debug/shuthost_coordinator")
            .to_string_lossy()
            .into_owned()
    });
    let child = Command::new(bin)
        .args(["control-service", "--config", tmp.to_str().unwrap()])
        .spawn()
        .expect("failed to start coordinator");
    // ensure child is killed on test end
    struct KillOnDrop(std::process::Child);
    impl Drop for KillOnDrop {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let _guard = KillOnDrop(child);

    // Wait for the server to start accepting connections (up to 20s).
    let start = std::time::Instant::now();
    loop {
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            break;
        }
        if start.elapsed() > Duration::from_secs(20) {
            panic!("server did not start listening within timeout");
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();

    // POST to /login with the correct token
    let url = format!("http://127.0.0.1:{port}/login");
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
    let protected = format!("http://127.0.0.1:{port}/api/hosts_status");
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
