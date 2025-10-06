use std::process::{Child, Command};
use std::time::{Duration, Instant};
use std::sync::Once;

pub fn get_free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("failed to bind to address")
        .local_addr()
        .unwrap()
        .port()
}

/// Guard that kills and waits on a child process when dropped.
pub struct KillOnDrop(pub Child);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn get_coordinator_bin() -> String {
    // Ensure build of all relevant binaries happened, then return the coordinator path.
    ensure_built();

    std::env::current_dir()
        .unwrap()
        .join("target/debug/shuthost_coordinator")
        .to_string_lossy()
        .into_owned()
}

/// Ensure the workspace binaries are built once per process. This builds all
/// binaries (no --bin) with the same flags used by tests to avoid concurrent
/// cargo builds from multiple helpers.
pub fn ensure_built() {
    static BUILD_ONCE_ALL: Once = Once::new();
    BUILD_ONCE_ALL.call_once(|| {
        let status = std::process::Command::new("cargo")
            .args(["build", "--no-default-features"])
            .status()
            .expect("failed to run cargo build for tests");
        assert!(status.success(), "cargo build failed");
    });
}

/// Spawn the coordinator service from a given config string.
/// Writes the config to a temp file and spawns the coordinator binary.
pub fn spawn_coordinator_with_config(port: u16, config_toml: &str) -> Child {
    let tmp = std::env::temp_dir().join(format!("integration_test_config_{}.toml", port));
    std::fs::write(&tmp, config_toml).expect("failed to write config");

    let bin = get_coordinator_bin();

    // Prefer built binary when running under `cargo test`.
    Command::new(bin)
        .args(["control-service", "--config", tmp.to_str().unwrap()])
        .spawn()
        .expect("failed to start coordinator")
}

/// Block until a TCP listener is accepting on `127.0.0.1:port` or timeout.
pub async fn wait_for_listening(port: u16, timeout_secs: u64) {
    let start = Instant::now();
    while std::net::TcpStream::connect(("127.0.0.1", port)).is_err() {
        if start.elapsed() > Duration::from_secs(timeout_secs) {
            panic!("server did not start within timeout");
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
