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

async fn spawn_coordinator_with_token(port: u16, token: &str) -> std::process::Child {
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
        "#,
    );
    let tmp = std::env::temp_dir().join(format!("integration_test_token_{}.toml", port));
    fs::write(&tmp, config).expect("failed to write config");

    let bin = std::env::var("CARGO_BIN_EXE_shuthost_coordinator").unwrap_or_else(|_| {
        std::env::current_dir()
            .unwrap()
            .join("target/debug/shuthost_coordinator")
            .to_string_lossy()
            .into_owned()
    });
    Command::new(bin)
        .args(["control-service", "--config", tmp.to_str().unwrap()])
        .spawn()
        .expect("failed to start coordinator")
}

#[tokio::test]
async fn insecure_post_redirects_with_insecure_error() {
    let port = get_free_port();
    let token = "correct-token";
    let child = spawn_coordinator_with_token(port, token).await;
    struct DropKill(std::process::Child);
    impl Drop for DropKill {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let _guard = DropKill(child);

    // wait for server to start
    let start = std::time::Instant::now();
    while std::net::TcpStream::connect(("127.0.0.1", port)).is_err() {
        if start.elapsed() > Duration::from_secs(10) {
            panic!("server did not start");
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
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
        .get(reqwest::header::LOCATION)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(
        loc.contains("error=insecure"),
        "location did not contain insecure error: {}",
        loc
    );
}

#[tokio::test]
async fn invalid_token_redirects_with_token_error() {
    let port = get_free_port();
    let token = "correct-token";
    let child = spawn_coordinator_with_token(port, token).await;
    struct DropKill(std::process::Child);
    impl Drop for DropKill {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let _guard = DropKill(child);

    // wait for server to start
    let start = std::time::Instant::now();
    while std::net::TcpStream::connect(("127.0.0.1", port)).is_err() {
        if start.elapsed() > Duration::from_secs(10) {
            panic!("server did not start");
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
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
        .get(reqwest::header::LOCATION)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(
        loc.contains("error=token"),
        "location did not contain token error: {}",
        loc
    );
}
