//! Configuration data types and structures for the coordinator.
//!
//! This module contains all the data structures used for configuration,
//! including host, client, server, TLS, and authentication settings.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents a configured host entry with network and security parameters.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
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
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Client {
    /// Shared secret used for authenticating callbacks.
    pub shared_secret: String,
}

/// HTTP server binding configuration section.
#[derive(Debug, Serialize, Deserialize, Default, Clone, PartialEq, Eq)]
pub struct ServerConfig {
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
pub struct TlsConfig {
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

/// Supported authentication modes for the Web UI
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
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

/// Authentication configuration wrapper
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
pub struct AuthConfig {
    #[serde(flatten)]
    pub mode: AuthMode,
    /// Optional base64-encoded cookie key (32 bytes). If omitted, a random key is generated.
    #[serde(default)]
    pub cookie_secret: Option<String>,
}

/// Root config structure for the coordinator, including server settings, hosts, and clients.
///
/// # Examples
///
/// ```
/// use coordinator::config::ControllerConfig;
/// let config = ControllerConfig::default();
/// assert!(config.hosts.is_empty());
/// assert!(config.clients.is_empty());
/// ```
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct ControllerConfig {
    /// HTTP server binding configuration.
    pub server: ServerConfig,
    /// Map of host identifiers to host configurations.
    pub hosts: HashMap<String, Host>,
    /// Map of client identifiers to client configurations.
    pub clients: HashMap<String, Client>,
}
