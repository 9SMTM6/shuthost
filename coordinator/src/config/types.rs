//! Configuration data types and structures for the coordinator.
//!
//! This module contains all the data structures used for configuration,
//! including host, client, server, TLS, and authentication settings.

use alloc::sync::Arc;
use std::{
    collections::HashMap,
    path::{Component, Path, PathBuf},
};

use secrecy::{ExposeSecret as _, SecretString};
use serde::Deserialize;

/// Represents a configured host entry with network and security parameters.
#[derive(Debug, Deserialize, Clone)]
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
    pub shared_secret: Arc<SecretString>,
    /// When `true`, the coordinator will periodically enforce the desired host state
    /// (derived from the current lease set) by sending wake or shutdown commands even
    /// if no lease change occurred.  Defaults to `false` (edge-triggered only).
    #[serde(default)]
    pub enforce_state: bool,
    /// Maximum seconds to wait for the host to come online after sending WoL packets.
    /// Defaults to [`DEFAULT_WAKE_TIMEOUT_SECS`](crate::app::host_control::DEFAULT_WAKE_TIMEOUT_SECS) when `None`.
    #[serde(default)]
    pub wake_timeout_secs: Option<u64>,
    /// Maximum seconds to wait for the host to go offline after sending a shutdown command.
    /// Defaults to [`DEFAULT_SHUTDOWN_TIMEOUT_SECS`](crate::app::host_control::DEFAULT_SHUTDOWN_TIMEOUT_SECS) when `None`.
    #[serde(default)]
    pub shutdown_timeout_secs: Option<u64>,
}

impl PartialEq for Host {
    fn eq(&self, other: &Self) -> bool {
        self.ip == other.ip
            && self.mac == other.mac
            && self.port == other.port
            && self.enforce_state == other.enforce_state
            && self.wake_timeout_secs == other.wake_timeout_secs
            && self.shutdown_timeout_secs == other.shutdown_timeout_secs
            && self.shared_secret.expose_secret() == other.shared_secret.expose_secret()
    }
}

/// Configuration for a client with its shared secret.
#[derive(Debug, Deserialize, Clone)]
pub(crate) struct Client {
    /// Shared secret used for authenticating callbacks.
    pub shared_secret: Arc<SecretString>,
}

impl PartialEq for Client {
    fn eq(&self, other: &Self) -> bool {
        self.shared_secret.expose_secret() == other.shared_secret.expose_secret()
    }
}

/// HTTP server binding configuration section.
#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub(crate) struct ServerConfig {
    /// TCP port for the web control service.
    pub port: u16,
    /// UDP port the coordinator listens on for agent startup broadcasts.
    pub broadcast_port: u16,
    /// Bind address for the HTTP listener.
    pub bind: String,
    /// Optional TLS configuration for serving HTTPS.
    pub tls: Option<TlsConfig>,
    /// Authentication configuration (defaults to no auth when omitted)
    pub auth: AuthConfig,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: 8080,
            bind: "127.0.0.1".to_string(),
            broadcast_port: shuthost_common::DEFAULT_COORDINATOR_BROADCAST_PORT,
            tls: None,
            auth: AuthConfig::default(),
        }
    }
}

/// TLS configuration for the HTTP server.
///
/// Paths in the config are interpreted relative to the config file when not absolute.
#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub(crate) struct TlsConfig {
    /// Optional path to a certificate PEM file. If present, enables TLS when paired with `key_path`.
    pub cert_path: String,

    /// Optional path to a private key PEM file. If present, enables TLS when paired with `cert_path`.
    pub key_path: String,

    /// When true (default), if no cert/key are provided a self-signed
    /// certificate will be generated and written next to the coordinator
    /// config so it persists across restarts.
    pub persist_self_signed: bool,
    /// Whether TLS is enabled. When false the server will serve plain HTTP even if the
    /// `tls` table is present. Defaults to true.
    pub enable: bool,
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self {
            cert_path: "./tls_cert.pem".to_string(),
            key_path: "./tls_key.pem".to_string(),
            persist_self_signed: true,
            enable: true,
        }
    }
}

/// Configuration for an optional local `SQLite` database.
#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub(crate) struct DbConfig {
    /// Path to the `SQLite` database file. Relative paths are resolved relative to the config file.
    pub path: String,
    /// Whether the local DB is enabled. When false the coordinator will act as if
    /// no DB is configured even if this table exists in the config file.
    pub enable: bool,
}

impl Default for DbConfig {
    fn default() -> Self {
        Self {
            path: "./shuthost.db".to_string(),
            enable: true,
        }
    }
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
pub fn resolve_config_relative_paths(config_path: &Path, relative_path: &str) -> PathBuf {
    let path = Path::new(relative_path);
    let resolved = if path.is_absolute() {
        path.to_path_buf()
    } else if relative_path == ":memory:" {
        // Special case: SQLite in-memory database path
        path.to_path_buf()
    } else {
        config_path
            .parent()
            .map_or_else(|| path.to_path_buf(), |d| d.join(path))
    };

    // Normalize the path to remove redundant ./ components
    // We can't use canonicalize() because the file might not exist yet
    normalize_path(&resolved)
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        use Component as C;
        match component {
            C::Normal(c) => {
                result.push(c);
            }
            C::ParentDir => {
                result.pop();
            }
            C::CurDir => {
                // Skip current directory components
            }
            C::RootDir | C::Prefix(_) => {
                result.push(component);
            }
        }
    }
    result
}

/// Stored OIDC configuration. Keeping this around allows the runtime to
/// rebuild the client if discovery needs to be retried - e.g. on JWKS failure.
#[derive(Debug, Deserialize, Clone)]
pub(crate) struct OidcConfig {
    pub issuer: String,
    #[serde(default = "default_oidc_client_id")]
    pub client_id: String,
    pub client_secret: Arc<SecretString>,
    #[serde(default = "default_oidc_scopes")]
    pub scopes: Vec<String>,
}

/// Supported authentication modes for the Web UI
#[derive(Debug, Deserialize, Clone, Default)]
#[serde(rename_all = "lowercase")]
pub(crate) enum AuthMode {
    /// No authentication, everything is public
    #[default]
    None,
    /// Simple token based auth. If token is not provided, a random token will be generated and logged on startup.
    /// The token persists across restarts when a database is configured, otherwise it's regenerated each startup.
    /// For security, the token is only logged during initial generation, not when loaded from database.
    Token { token: Option<Arc<SecretString>> },
    /// `OpenID` Connect login via authorization code flow
    Oidc(OidcConfig),
    /// External auth was configured (reverse proxy / external provider). The
    /// `exceptions_version` is used to record which set/level of exceptions the
    /// operator acknowledged; the UI will show a warning when this doesn't
    /// match the current expected version so operators can update their proxy
    /// rules.
    External { exceptions_version: u32 },
}

impl PartialEq for AuthMode {
    fn eq(&self, other: &Self) -> bool {
        use AuthMode as AM;
        match (self, other) {
            (&AM::None, &AM::None) => true,
            (&AM::Token { token: ref t1 }, &AM::Token { token: ref t2 }) => match (t1, t2) {
                (&Some(ref s1), &Some(ref s2)) => s1.expose_secret() == s2.expose_secret(),
                (&None, &None) => true,
                _ => false,
            },
            (&AM::Oidc(ref cfg1), &AM::Oidc(ref cfg2)) => {
                cfg1.issuer == cfg2.issuer
                    && cfg1.client_id == cfg2.client_id
                    && cfg1.client_secret.expose_secret() == cfg2.client_secret.expose_secret()
                    && cfg1.scopes == cfg2.scopes
            }
            (
                &AM::External {
                    exceptions_version: v1,
                },
                &AM::External {
                    exceptions_version: v2,
                },
            ) => v1 == v2,
            _ => false,
        }
    }
}

// Defaults for OIDC fields used by serde(default = ...)
fn default_oidc_scopes() -> Vec<String> {
    vec!["openid".to_string(), "profile".to_string()]
}

fn default_oidc_client_id() -> String {
    "shuthost".to_string()
}

/// Authentication configuration wrapper
#[derive(Debug, Deserialize, Clone, Default)]
pub(crate) struct AuthConfig {
    #[serde(flatten)]
    pub mode: AuthMode,
    /// Optional base64-encoded cookie key (32 bytes). If omitted, a random key is generated and persisted to database if available.
    #[serde(default)]
    pub cookie_secret: Option<Arc<SecretString>>,
}

impl PartialEq for AuthConfig {
    fn eq(&self, other: &Self) -> bool {
        self.mode == other.mode && {
            match (&self.cookie_secret, &other.cookie_secret) {
                (&Some(ref s1), &Some(ref s2)) => s1.expose_secret() == s2.expose_secret(),
                (&None, &None) => true,
                _ => false,
            }
        }
    }
}

/// Root config structure for the coordinator, including server settings, hosts, and clients.
/// ```
#[derive(Debug, Deserialize, Default, Clone, PartialEq)]
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
