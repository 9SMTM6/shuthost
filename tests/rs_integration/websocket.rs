//! Integration tests for websocket functionality

use core::time::Duration;
use std::env;

use futures_util::StreamExt as _;
use shuthost_coordinator::{
    WsMessage,
    app::HostState,
    websocket::{DynamicConfig, FrontendHookAction},
};
use tokio::{fs, time};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::common::{
    get_free_port, runtime_test_config, spawn_coordinator_with_config,
    spawn_coordinator_with_config_file, spawn_host_agent_default, wait_for_listening,
};

#[tokio::test]
#[expect(
    clippy::too_many_lines,
    reason = "Integration tests sometimes need many assertions, but whitelisting this lint for all tests feels too broad.",
)]
async fn websocket_config_updates() {
    let port = get_free_port();
    let shared_secret = "secret";
    let config_path = env::temp_dir().join(format!("ws_test_config_{port}.toml"));
    let initial_config = format!(
        r#"
        [server]
        port = {port}
        bind = "127.0.0.1"

        [hosts]

        [clients]
    "#
    );
    fs::write(&config_path, &initial_config)
        .await
        .expect("failed to write config");

    let _child = spawn_coordinator_with_config_file(&config_path, port);
    wait_for_listening(port, 5).await;

    // Connect websocket client
    let url = format!("ws://127.0.0.1:{port}/ws");
    let (ws_stream, _) = connect_async(url)
        .await
        .expect("failed to connect websocket");
    let (_write, mut read) = ws_stream.split();

    // Read the initial message
    let initial_msg = read.next().await.unwrap().unwrap();
    let initial: WsMessage = serde_json::from_str(&initial_msg.to_string()).unwrap();
    match initial {
        WsMessage::Initial(initial) => {
            assert!(initial.dynamic_config.hosts.is_empty());
            assert!(initial.dynamic_config.clients.is_empty());
            assert!(initial.dynamic_config.host_config_map.is_empty());
        }
        _ => panic!("Expected Initial message"),
    }

    // Modify config to add a host
    let updated_config = format!(
        r#"
        [server]
        port = {port}
        bind = "127.0.0.1"

        [hosts.newhost]
        ip = "192.168.1.1"
        mac = "00:11:22:33:44:55"
        port = 8080
        shared_secret = "{shared_secret}"

        [hosts.newhost.pre_startup]
        type = "http"
        url = "https://example.com/pre-startup"
        method = "GET"
        delay_secs = 2
        timeout_secs = 5

        [hosts.newhost.post_shutdown]
        type = "exec"
        program = "/usr/bin/shutdown"
        delay_secs = 1
        timeout_secs = 10

        [clients]
    "#
    );
    fs::write(&config_path, &updated_config)
        .await
        .expect("failed to update config");

    // Wait for ConfigChanged message
    let mut config_changed_received = false;
    let timeout = time::timeout(Duration::from_secs(10), async {
        while let Some(msg) = read.next().await {
            let msg = msg.unwrap();
            if let Message::Text(text) = msg {
                let ws_msg: WsMessage = serde_json::from_str(&text).unwrap();
                if let WsMessage::ConfigChanged(DynamicConfig {
                    clients,
                    hosts,
                    host_config_map,
                }) = ws_msg
                {
                    assert_eq!(hosts, vec!["newhost".to_string()]);
                    assert!(clients.is_empty());
                    assert_eq!(host_config_map.len(), 1);
                    let host_config = &host_config_map["newhost"];
                    assert!(!host_config.enforce_state);

                    let pre_hook = host_config
                        .pre_startup
                        .as_ref()
                        .expect("pre_startup hook should be present");
                    assert_eq!(pre_hook.delay_secs, 2);
                    assert_eq!(pre_hook.timeout_secs, 5);
                    match pre_hook.action {
                        FrontendHookAction::Http {
                            ref url,
                            ref method,
                        } => {
                            assert_eq!(url, "https://example.com/pre-startup");
                            assert_eq!(method, "GET");
                        }
                        FrontendHookAction::Exec { .. } => {
                            panic!("Expected pre_startup HTTP hook")
                        }
                    }

                    let post_hook = host_config
                        .post_shutdown
                        .as_ref()
                        .expect("post_shutdown hook should be present");
                    assert_eq!(post_hook.delay_secs, 1);
                    assert_eq!(post_hook.timeout_secs, 10);
                    match post_hook.action {
                        FrontendHookAction::Exec { ref program } => {
                            assert_eq!(program, "/usr/bin/shutdown");
                        }
                        FrontendHookAction::Http { .. } => {
                            panic!("Expected post_shutdown exec hook")
                        }
                    }

                    config_changed_received = true;
                    break;
                }
            }
        }
    })
    .await;

    assert!(timeout.is_ok(), "Timeout waiting for ConfigChanged message");

    assert!(config_changed_received);
}

#[tokio::test]
async fn websocket_host_status_changes() {
    let coord_port = get_free_port();
    let agent_port = get_free_port();
    let shared_secret = "testsecret";

    let _coordinator_child = spawn_coordinator_with_config(
        coord_port,
        &(format!(
            r#"
        [server]
        port = {coord_port}
        bind = "127.0.0.1"

        [hosts.testhost]
        ip = "127.0.0.1"
        mac = "00:11:22:33:44:55"
        port = {agent_port}
        shared_secret = "{shared_secret}"

        [clients]
    "#
        ) + &runtime_test_config()),
    );
    wait_for_listening(coord_port, 5).await;

    // Connect websocket client
    let url = format!("ws://127.0.0.1:{coord_port}/ws");
    let (ws_stream, _) = connect_async(url)
        .await
        .expect("failed to connect websocket");
    let (_write, mut read) = ws_stream.split();

    // Read the initial message
    let initial_msg = read.next().await.unwrap().unwrap();
    let initial: WsMessage = serde_json::from_str(&initial_msg.to_string()).unwrap();
    match initial {
        WsMessage::Initial(initial) => {
            // Initially, host should be offline
            assert_eq!(
                initial.status_map.get("testhost"),
                Some(&HostState::Offline)
            );
        }
        _ => panic!("Expected Initial message"),
    }

    // Start the host agent
    let agent = spawn_host_agent_default(shared_secret, agent_port);

    // Wait for host to come online
    let mut online_received = false;
    let timeout = time::timeout(Duration::from_secs(10), async {
        while let Some(msg) = read.next().await {
            let msg = msg.unwrap();
            if let Message::Text(text) = msg {
                let ws_msg: WsMessage = serde_json::from_str(&text).unwrap();
                if let WsMessage::HostStatus(status) = ws_msg
                    && status.get("testhost") == Some(&HostState::Online)
                {
                    online_received = true;
                    break;
                }
            }
        }
    })
    .await;

    assert!(timeout.is_ok(), "Timeout waiting for host to come online");

    assert!(online_received, "Host should have come online");

    // Now kill the agent
    drop(agent); // This kills the agent

    // Wait for host to go offline
    let mut offline_received = false;
    let timeout = time::timeout(Duration::from_secs(10), async {
        while let Some(msg) = read.next().await {
            let msg = msg.unwrap();
            if let Message::Text(text) = msg {
                let ws_msg: WsMessage = serde_json::from_str(&text).unwrap();
                if let WsMessage::HostStatus(status) = ws_msg
                    && status.get("testhost") == Some(&HostState::Offline)
                {
                    offline_received = true;
                    break;
                }
            }
        }
    })
    .await;

    assert!(timeout.is_ok(), "Timeout waiting for host to go offline");

    assert!(offline_received, "Host should have gone offline");
}
