//! Configuration data types and structures for the coordinator.
//!
//! This module contains all the data structures used for configuration,
//! including host, client, server, TLS, and authentication settings.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Represents a configured host entry with network and security parameters.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub(crate) struct Host {
    /// IP address of the host agent.
    pub ip: String,
    /// MAC address of the host agent's network interface, required for WOL.
    /// There is an undocumented feature where setting this to disableWOL disables waking per WOL.
    /// In the future we may offer alternative wake options, then this will be documented,
    /// as of now this is primarily for tests
    pub mac: String,
    /// TCP port the host agent listens on.
    pub port: u16,
    /// Shared secret for HMAC authentication.
    pub shared_secret: String,
}

/// Configuration for a client with its shared secret.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub(crate) struct Client {
    /// Shared secret used for authenticating callbacks.
    pub shared_secret: String,
}

/// HTTP server binding configuration section.
#[derive(Debug, Serialize, Deserialize, Default, Clone, PartialEq, Eq)]
pub(crate) struct ServerConfig {
    /// TCP port for the web control service.
    pub port: u16,
    /// Bind address for the HTTP listener.
    pub bind: String,
    /// Optional TLS configuration for serving HTTPS.
    #[serde(default)]
    pub tls: Option<TlsConfig>,
    /// Authentication configuration (defaults to no auth when omitted)
    #[serde(default)]
    pub auth: AuthConfig,
}

/// TLS configuration for the HTTP server.
///
/// Paths in the config are interpreted relative to the config file when not absolute.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub(crate) struct TlsConfig {
    /// Optional path to a certificate PEM file. If present, enables TLS when paired with `key_path`.
    #[serde(default = "relative_cert_path")]
    pub cert_path: String,

    /// Optional path to a private key PEM file. If present, enables TLS when paired with `cert_path`.
    #[serde(default = "relative_key_path")]
    pub key_path: String,

    /// When true (default), if no cert/key are provided a self-signed
    /// certificate will be generated and written next to the coordinator
    /// config so it persists across restarts.
    #[serde(default = "do_persist_self_signed")]
    pub persist_self_signed: bool,
    /// Whether TLS is enabled. When false the server will serve plain HTTP even if the
    /// `tls` table is present. Defaults to true.
    #[serde(default = "do_tls_enable")]
    pub enable: bool,
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self {
            cert_path: relative_cert_path(),
            key_path: relative_key_path(),
            persist_self_signed: do_persist_self_signed(),
            enable: do_tls_enable(),
        }
    }
}

/// Configuration for an optional local SQLite database.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub(crate) struct DbConfig {
    /// Path to the SQLite database file. Relative paths are resolved relative to the config file.
    #[serde(default = "default_db_path")]
    pub path: String,
    /// Whether the local DB is enabled. When false the coordinator will act as if
    /// no DB is configured even if this table exists in the config file.
    #[serde(default = "do_db_enable")]
    pub enable: bool,
}

impl Default for DbConfig {
    fn default() -> Self {
        Self {
            path: default_db_path(),
            enable: do_db_enable(),
        }
    }
}

fn relative_cert_path() -> String {
    // Relative default path next to config file (must not be empty)
    "./tls_cert.pem".to_string()
}

fn relative_key_path() -> String {
    "./tls_key.pem".to_string()
}

const fn do_persist_self_signed() -> bool {
    true
}

const fn do_tls_enable() -> bool {
    true
}

fn default_db_path() -> String {
    "./shuthost.db".to_string()
}

const fn do_db_enable() -> bool {
    true
}

/// Resolves a path to an absolute one.
///
/// If the path is absolute, returns it as-is. If relative, joins it with the
/// config file's parent directory and normalizes the result to remove redundant
/// components like `./`.
///
/// # Arguments
///
/// * `config_path` - Path to the config file
/// * `relative_path` - Path to resolve (may be absolute or relative)
///
/// # Returns
///
/// A normalized absolute path
pub fn resolve_config_relative_paths(
    config_path: &std::path::Path,
    relative_path: &str,
) -> std::path::PathBuf {
    let path = std::path::Path::new(relative_path);
    let resolved = if path.is_absolute() {
        path.to_path_buf()
    } else if relative_path == ":memory:" {
        // Special case: SQLite in-memory database path
        path.to_path_buf()
    } else {
        config_path
            .parent()
            .map(|d| d.join(path))
            .unwrap_or_else(|| path.to_path_buf())
    };

    // Normalize the path to remove redundant ./ components
    // We can't use canonicalize() because the file might not exist yet
    normalize_path(&resolved)
}

fn normalize_path(path: &std::path::Path) -> std::path::PathBuf {
    let mut result = std::path::PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::Normal(c) => {
                result.push(c);
            }
            std::path::Component::ParentDir => {
                result.pop();
            }
            std::path::Component::CurDir => {
                // Skip current directory components
            }
            std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                result.push(component);
            }
        }
    }
    result
}

/// Supported authentication modes for the Web UI
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum AuthMode {
    /// No authentication, everything is public
    #[default]
    None,
    /// Simple token based auth. If token is not provided, a random token will be generated and logged on startup.
    /// The token persists across restarts when a database is configured, otherwise it's regenerated each startup.
    /// For security, the token is only logged during initial generation, not when loaded from database.
    Token { token: Option<String> },
    /// OpenID Connect login via authorization code flow
    Oidc {
        issuer: String,
        #[serde(default = "default_oidc_client_id")]
        client_id: String,
        client_secret: String,
        #[serde(default = "default_oidc_scopes")]
        scopes: Vec<String>,
    },
    /// External auth was configured (reverse proxy / external provider). The
    /// `exceptions_version` is used to record which set/level of exceptions the
    /// operator acknowledged; the UI will show a warning when this doesn't
    /// match the current expected version so operators can update their proxy
    /// rules.
    External { exceptions_version: u32 },
}

// Defaults for OIDC fields used by serde(default = ...)
fn default_oidc_scopes() -> Vec<String> {
    vec!["openid".to_string(), "profile".to_string()]
}

fn default_oidc_client_id() -> String {
    "shuthost".to_string()
}

/// Authentication configuration wrapper
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
pub(crate) struct AuthConfig {
    #[serde(flatten)]
    pub mode: AuthMode,
    /// Optional base64-encoded cookie key (32 bytes). If omitted, a random key is generated and persisted to database if available.
    #[serde(default)]
    pub cookie_secret: Option<String>,
}

/// Root config structure for the coordinator, including server settings, hosts, and clients.
/// ```
#[derive(Debug, Serialize, Deserialize, Default, Clone, PartialEq, Eq)]
pub(crate) struct ControllerConfig {
    /// HTTP server binding configuration.
    pub server: ServerConfig,
    /// Map of host identifiers to host configurations.
    pub hosts: HashMap<String, Host>,
    /// Map of client identifiers to client configurations.
    pub clients: HashMap<String, Client>,
    /// Optional top-level database configuration. When omitted DB persistence is disabled.
    #[serde(default)]
    pub db: Option<DbConfig>,
}
