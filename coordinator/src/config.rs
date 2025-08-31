//! Configuration management for the coordinator: loading and watching the TOML config file.

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::{mpsc::unbounded_channel, watch};
use tracing::{error, info};

/// Represents a configured host entry with network and security parameters.
///
/// # Examples
///
/// ```
/// use shuthost_coordinator::config::Host;
/// let host = Host {
///     ip: "127.0.0.1".to_string(),
///     mac: "aa:bb:cc:dd:ee:ff".to_string(),
///     port: 8080,
///     shared_secret: "secret".to_string(),
/// };
/// assert_eq!(host.ip, "127.0.0.1");
/// assert_eq!(host.mac, "aa:bb:cc:dd:ee:ff");
/// assert_eq!(host.port, 8080);
/// assert_eq!(host.shared_secret, "secret");
/// ```
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Host {
    /// IP address of the host agent.
    pub ip: String,
    /// MAC address of the host agent's network interface.
    pub mac: String,
    /// TCP port the host agent listens on.
    pub port: u16,
    /// Shared secret for HMAC authentication.
    pub shared_secret: String,
}

/// Configuration for a client with its shared secret.
///
/// # Examples
///
/// ```
/// use shuthost_coordinator::config::Client;
/// let client = Client { shared_secret: "secret".to_string() };
/// assert_eq!(client.shared_secret, "secret");
/// ```
#[derive(Debug, Serialize, Deserialize)]
pub struct Client {
    /// Shared secret used for authenticating callbacks.
    pub shared_secret: String,
}

/// Root config structure for the coordinator, including server settings, hosts, and clients.
///
/// # Examples
///
/// ```
/// use shuthost_coordinator::config::ControllerConfig;
/// let config = ControllerConfig::default();
/// assert!(config.hosts.is_empty());
/// assert!(config.clients.is_empty());
/// ```
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ControllerConfig {
    /// HTTP server binding configuration.
    pub server: ServerConfig,
    /// Map of host identifiers to host configurations.
    pub hosts: HashMap<String, Host>,
    /// Map of client identifiers to client configurations.
    pub clients: HashMap<String, Client>,
}

/// HTTP server binding configuration section.
///
/// # Examples
///
/// ```
/// use shuthost_coordinator::config::ServerConfig;
/// let sc = ServerConfig::default();
/// assert_eq!(sc.port, 0);
/// assert_eq!(sc.bind, "");
/// ```
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ServerConfig {
    /// TCP port for the web control service.
    pub port: u16,
    /// Bind address for the HTTP listener.
    pub bind: String,
    /// Authentication configuration (defaults to no auth when omitted)
    #[serde(default)]
    pub auth: AuthConfig,
}

/// Supported authentication modes for the Web UI
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum AuthMode {
    /// No authentication, everything is public
    #[default]
    None,
    /// Simple bearer token based auth. If token is not provided, a random token will be generated and logged on startup.
    Token { token: Option<String> },
    /// OpenID Connect login via authorization code flow
    Oidc {
        issuer: String,
        client_id: String,
        client_secret: String,
        #[serde(default = "default_oidc_scopes")]
        scopes: Vec<String>,
        /// The callback path on this server (defaults to /auth/callback)
        #[serde(default = "default_redirect_path")]
        redirect_path: String,
    },
}

// Defaults for OIDC fields used by serde(default = ...)
fn default_oidc_scopes() -> Vec<String> {
    vec!["openid".to_string(), "profile".to_string()]
}

fn default_redirect_path() -> String {
    "/auth/callback".to_string()
}

/// Authentication configuration wrapper
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct AuthConfig {
    #[serde(flatten)]
    pub mode: AuthMode,
    /// Optional base64-encoded cookie key (32 bytes). If omitted, a random key is generated.
    #[serde(default)]
    pub cookie_secret: Option<String>,
}

/// Reads and parses the coordinator config from a TOML file.
///
/// # Arguments
///
/// * `path` - File path to the TOML configuration file.
///
/// # Returns
///
/// A `ControllerConfig` on success, or an error boxed trait object.
pub async fn load_coordinator_config<P: AsRef<Path>>(
    path: P,
) -> Result<ControllerConfig, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path).await?;
    let config: ControllerConfig = toml::from_str(&content)?;
    Ok(config)
}

/// Handles the logic for reloading the configuration file and updating the application state.
///
/// This function is called when a file modification event is detected. It loads the new
/// configuration, checks for unsupported changes (like port or bind address), and sends the
/// new configuration to the application's state management channel.
///
/// # Arguments
///
/// * `path` - The path to the configuration file.
/// * `tx` - The sender part of a watch channel for broadcasting configuration updates.
/// * `initial_port` - The server port at application startup, used to detect changes.
/// * `initial_bind` - The server bind address at application startup, used to detect changes.
async fn validate_and_broadcast_config_change(
    path: &Path,
    tx: &watch::Sender<Arc<ControllerConfig>>,
    initial_config: &ControllerConfig,
) {
    info!("Config file modified. Reloading...");
    match load_coordinator_config(path).await {
        Ok(new_config) => {
            let initial_port = initial_config.server.port;
            if new_config.server.port != initial_port {
                error!(
                    "Port change detected in config file. Changing ports while the server is running is not supported. Server will continue to run on port {}",
                    initial_port
                );
            }
            let initial_bind = &initial_config.server.bind;
            if new_config.server.bind != *initial_bind {
                error!(
                    "Bind address change detected in config file. Changing bind address while the server is running is not supported. Server will continue to run on {}",
                    initial_bind
                );
            }
            if tx.send(Arc::new(new_config)).is_err() {
                error!("Failed to send updated config through watch channel");
                return;
            }
            info!("Config reloaded.");
        }
        Err(e) => {
            error!("Failed to reload config: {}", e);
        }
    }
}

/// Watches a config file for modifications and updates the provided channel on changes.
///
/// # Arguments
///
/// * `path` - Path to the config file to watch.
/// * `tx` - Watch channel sender to broadcast new config instances.
pub async fn watch_config_file(path: std::path::PathBuf, tx: watch::Sender<Arc<ControllerConfig>>) {
    let (raw_tx, mut raw_rx) = unbounded_channel::<Event>();
    let initial_config = tx.borrow().clone();

    let mut watcher = RecommendedWatcher::new(
        move |res| {
            if let Ok(event) = res
                && raw_tx.send(event).is_err()
            {
                error!("Failed to send event to config watcher channel");
            }
        },
        notify::Config::default(),
    )
    .expect("Failed to create file watcher");

    watcher
        .watch(&path, RecursiveMode::NonRecursive)
        .expect("Failed to watch config file");

    while let Some(event) = raw_rx.recv().await {
        if matches!(event.kind, EventKind::Modify(_)) {
            validate_and_broadcast_config_change(&path, &tx, &initial_config).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_load_coordinator_config_file() {
        let toml_str = r#"
            [server]
            port = 9090
            bind = "0.0.0.0"

            [hosts.foo]
            ip = "1.2.3.4"
            mac = "aa:aa:aa:aa:aa:aa"
            port = 5678
            shared_secret = "s1"

            [clients.bar]
            shared_secret = "s2"
        "#;
        let tmp = std::env::temp_dir().join("test_config.toml");
        std::fs::write(&tmp, toml_str).unwrap();
        let cfg = load_coordinator_config(&tmp).await.unwrap();
        assert_eq!(cfg.server.port, 9090);
        assert_eq!(cfg.server.bind, "0.0.0.0");
        let host = cfg.hosts.get("foo").unwrap();
        assert_eq!(host.ip, "1.2.3.4");
        assert_eq!(host.mac, "aa:aa:aa:aa:aa:aa");
        assert_eq!(host.port, 5678);
        assert_eq!(host.shared_secret, "s1");
        let client = cfg.clients.get("bar").unwrap();
        assert_eq!(client.shared_secret, "s2");
    }

    #[test]
    fn test_deserialize_controller_config() {
        let toml_str = r#"
            [server]
            port = 8080
            bind = "127.0.0.1"

            [hosts.foo]
            ip = "192.168.0.2"
            mac = "aa:bb:cc:dd:ee:ff"
            port = 1234
            shared_secret = "secret1"

            [clients.bar]
            shared_secret = "secret2"
        "#;
        let cfg: ControllerConfig = toml::from_str(toml_str).expect("Failed to parse TOML");
        assert_eq!(cfg.server.port, 8080);
        assert_eq!(cfg.server.bind, "127.0.0.1");
        assert_eq!(cfg.hosts.len(), 1);
        let host = cfg.hosts.get("foo").expect("Missing host foo");
        assert_eq!(host.ip, "192.168.0.2");
        assert_eq!(host.mac, "aa:bb:cc:dd:ee:ff");
        assert_eq!(host.port, 1234);
        assert_eq!(host.shared_secret, "secret1");
        assert_eq!(cfg.clients.len(), 1);
        let client = cfg.clients.get("bar").expect("Missing client bar");
        assert_eq!(client.shared_secret, "secret2");
    }

    #[tokio::test]
    async fn test_load_coordinator_config_missing_file() {
        let tmp = std::env::temp_dir().join("does_not_exist.toml");
        let res = load_coordinator_config(&tmp).await;
        assert!(res.is_err(), "Expected error for missing file");
    }

    #[tokio::test]
    async fn test_load_coordinator_config_invalid_toml() {
        let tmp = std::env::temp_dir().join("invalid.toml");
        std::fs::write(&tmp, "not valid toml").unwrap();
        let res = load_coordinator_config(&tmp).await;
        assert!(res.is_err(), "Expected error for invalid TOML");
    }
}
