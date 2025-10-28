//! Integration tests for websocket functionality

use std::time::Duration;

use futures_util::StreamExt;
use shuthost_coordinator::WsMessage;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::common::{
    get_free_port, spawn_coordinator_with_config, spawn_coordinator_with_config_file,
    spawn_host_agent_default, wait_for_listening,
};

#[tokio::test]
async fn test_websocket_config_updates() {
    let port = get_free_port();
    let shared_secret = "secret";
    let config_path = std::env::temp_dir().join(format!("ws_test_config_{}.toml", port));
    let initial_config = format!(
        r#"
        [server]
        port = {}
        bind = "127.0.0.1"

        [hosts]

        [clients]
    "#,
        port
    );
    std::fs::write(&config_path, &initial_config).expect("failed to write config");

    let _child = spawn_coordinator_with_config_file(&config_path);
    wait_for_listening(port, 5).await;

    // Connect websocket client
    let url = format!("ws://127.0.0.1:{}/ws", port);
    let (ws_stream, _) = connect_async(url)
        .await
        .expect("failed to connect websocket");
    let (_write, mut read) = ws_stream.split();

    // Read the initial message
    let initial_msg = read.next().await.unwrap().unwrap();
    let initial: WsMessage = serde_json::from_str(&initial_msg.to_string()).unwrap();
    match initial {
        WsMessage::Initial { hosts, clients, .. } => {
            assert!(hosts.is_empty());
            assert!(clients.is_empty());
        }
        _ => panic!("Expected Initial message"),
    }

    // Modify config to add a host
    let updated_config = format!(
        r#"
        [server]
        port = {}
        bind = "127.0.0.1"

        [hosts.newhost]
        ip = "192.168.1.1"
        mac = "00:11:22:33:44:55"
        port = 8080
        shared_secret = "{shared_secret}"

        [clients]
    "#,
        port
    );
    std::fs::write(&config_path, &updated_config).expect("failed to update config");

    // Wait for ConfigChanged message
    let mut config_changed_received = false;
    let timeout = tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(msg) = read.next().await {
            let msg = msg.unwrap();
            if let Message::Text(text) = msg {
                let ws_msg: WsMessage = serde_json::from_str(&text).unwrap();
                if let WsMessage::ConfigChanged { hosts, clients } = ws_msg {
                    assert_eq!(hosts, vec!["newhost".to_string()]);
                    assert!(clients.is_empty());
                    config_changed_received = true;
                    break;
                }
            }
        }
    })
    .await;

    if timeout.is_err() {
        panic!("Timeout waiting for ConfigChanged message");
    }

    assert!(config_changed_received);
}

#[tokio::test]
async fn test_websocket_host_status_changes() {
    let coord_port = get_free_port();
    let agent_port = get_free_port();
    let shared_secret = "testsecret";

    let _coordinator_child = spawn_coordinator_with_config(
        coord_port,
        &format!(
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
        ),
    );
    wait_for_listening(coord_port, 5).await;

    // Connect websocket client
    let url = format!("ws://127.0.0.1:{}/ws", coord_port);
    let (ws_stream, _) = connect_async(url)
        .await
        .expect("failed to connect websocket");
    let (_write, mut read) = ws_stream.split();

    // Read the initial message
    let initial_msg = read.next().await.unwrap().unwrap();
    let initial: WsMessage = serde_json::from_str(&initial_msg.to_string()).unwrap();
    match initial {
        WsMessage::Initial { status, .. } => {
            // Initially, host should be offline
            assert_eq!(status.get("testhost"), Some(&false));
        }
        _ => panic!("Expected Initial message"),
    }

    // Start the host agent
    let agent = spawn_host_agent_default(shared_secret, agent_port);

    // Wait for host to come online
    let mut online_received = false;
    let timeout = tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(msg) = read.next().await {
            let msg = msg.unwrap();
            if let Message::Text(text) = msg {
                let ws_msg: WsMessage = serde_json::from_str(&text).unwrap();
                if let WsMessage::HostStatus(status) = ws_msg {
                    if status.get("testhost") == Some(&true) {
                        online_received = true;
                        break;
                    }
                }
            }
        }
    })
    .await;

    if timeout.is_err() {
        panic!("Timeout waiting for host to come online");
    }

    assert!(online_received, "Host should have come online");

    // Now kill the agent
    drop(agent); // This kills the agent

    // Wait for host to go offline
    let mut offline_received = false;
    let timeout = tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(msg) = read.next().await {
            let msg = msg.unwrap();
            if let Message::Text(text) = msg {
                let ws_msg: WsMessage = serde_json::from_str(&text).unwrap();
                if let WsMessage::HostStatus(status) = ws_msg {
                    if status.get("testhost") == Some(&false) {
                        offline_received = true;
                        break;
                    }
                }
            }
        }
    })
    .await;

    if timeout.is_err() {
        panic!("Timeout waiting for host to go offline");
    }

    assert!(offline_received, "Host should have gone offline");
}
