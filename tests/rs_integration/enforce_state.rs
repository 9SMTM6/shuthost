//! Integration tests validating enforcement of host state (shutdown path).
//!
//! These tests spawn a real coordinator and host agent and then observe a
//! tangible side effect (a file written by the agent's shutdown command) to
//! determine whether the coordinator's enforcer task acted when it shouldn't
//! (`enforce_state=false`) or should (`enforce_state=true`).

use core::time::Duration;
use std::{env, fs};

use crate::common::{
    get_free_port, spawn_coordinator_with_config, spawn_host_agent, wait_for_agent_ready,
    wait_for_host_state, wait_for_listening,
};
use secrecy::SecretString;
use shuthost_coordinator::app::{ENFORCE_STABILIZATION_THRESHOLD, HostState};
use tokio::time;

const SLEEP_EXTRA: Duration = Duration::from_secs(2);

async fn run_enforce_test(enforce: bool) -> bool {
    let coord_port = get_free_port();
    let agent_port = get_free_port();
    let secret = "secret123";

    let config = format!(
        r#"
        [server]
        port = {coord_port}
        bind = "127.0.0.1"

        [hosts.foo]
        ip = "127.0.0.1"
        mac = "00:11:22:33:44:55"
        port = {agent_port}
        shared_secret = "{secret}"
        enforce_state = {enforce}

        [clients]
    "#
    );

    let _coord = spawn_coordinator_with_config(coord_port, &config);
    wait_for_listening(coord_port, 5).await;

    let shutdown_file = env::temp_dir().join(format!("enforce_state_{agent_port}.tmp"));
    // ensure it doesn't exist before starting
    drop(fs::remove_file(&shutdown_file));

    let _agent = spawn_host_agent(
        secret,
        agent_port,
        shuthost_common::DEFAULT_COORDINATOR_BROADCAST_PORT,
        &format!("echo STOP > {}", shutdown_file.display()),
    );

    wait_for_agent_ready(agent_port, &SecretString::from(secret), 5).await;

    assert!(
        wait_for_host_state(coord_port, "foo", HostState::Online, 10).await,
        "host should appear online before enforcement period"
    );

    // wait long enough for the enforcer to notice the mismatch
    time::sleep(ENFORCE_STABILIZATION_THRESHOLD + SLEEP_EXTRA).await;

    let exists = shutdown_file.exists();
    drop(fs::remove_file(&shutdown_file));
    exists
}

#[tokio::test]
async fn enforce_state_triggers_shutdown_when_host_manual_online() {
    let result = run_enforce_test(true).await;
    assert!(
        result,
        "shutdown file should be created when enforce_state=true"
    );
}

#[tokio::test]
async fn no_enforce_state_does_not_shutdown_manual_online_host() {
    let result = run_enforce_test(false).await;
    assert!(
        !result,
        "shutdown file must NOT be created when enforce_state=false"
    );
}
