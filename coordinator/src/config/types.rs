//! Configuration data types and structures for the coordinator.
//!
//! This module contains all the data structures used for configuration,
//! including host, client, server, TLS, and authentication settings.

use alloc::sync::Arc;
use std::{
    collections::HashMap,
    path::{Component, Path, PathBuf},
};

use reqwest::Method;
use secrecy::{ExposeSecret as _, SecretString};
use serde::{Deserialize, de};

/// Action to execute as a pre-startup or post-shutdown hook.
#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum HookAction {
    /// Directly execute a program with arguments — no shell involved.
    Exec {
        /// The program to execute (path or name resolvable via `PATH`).
        program: String,
        /// Arguments to pass to the program.
        #[serde(default)]
        args: Vec<String>,
    },
    /// Send an HTTP request. See the example config for timing caveats with pre-startup hooks.
    Http {
        /// The URL to send the request to.
        url: reqwest::Url,
        /// HTTP method. Defaults to `POST` when omitted.
        /// Validated at parse time — an invalid method string is a configuration error.
        #[serde(default = "POST", deserialize_with = "deserialize_http_method")]
        method: Method,
        /// Optional request body sent as a raw string.
        #[serde(default)]
        body: Option<String>,
    },
}

#[expect(non_snake_case, reason = "Used as serde(default)")]
const fn POST() -> Method {
    Method::POST
}

/// Deserializes an optional HTTP method string, validating it at parse time.
fn deserialize_http_method<'de, D>(de: D) -> Result<reqwest::Method, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(de)?;
    reqwest::Method::from_bytes(s.as_bytes()).map_err(de::Error::custom)
}

/// Configuration for a single pre-startup or post-shutdown hook.
#[derive(Debug, Deserialize, Clone, PartialEq)]
pub(crate) struct HookConfig {
    /// The action to execute.
    #[serde(flatten)]
    pub action: HookAction,
    /// Timeout in seconds for this hook. Defaults to 30 seconds when omitted.
    #[serde(default)]
    pub timeout_secs: Option<u64>,
}

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
    /// Maximum seconds to wait for the host to come online after sending `WoL` packets.
    /// When `None`, the runtime-configured default wake timeout is used.
    #[serde(default)]
    pub wake_timeout_secs: Option<u64>,
    /// Maximum seconds to wait for the host to go offline after sending a shutdown command.
    /// When `None`, the runtime-configured default shutdown timeout is used.
    #[serde(default)]
    pub shutdown_timeout_secs: Option<u64>,
    /// Optional hook to execute before sending the wake-on-LAN packet.
    #[serde(default)]
    pub pre_startup: Option<HookConfig>,
    /// Optional hook to execute after the host is confirmed offline.
    #[serde(default)]
    pub post_shutdown: Option<HookConfig>,
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
            && self.pre_startup == other.pre_startup
            && self.post_shutdown == other.post_shutdown
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

/// Runtime tuning parameters for the coordinator.
///
/// These are read once at startup (restart required to change them).
#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub(crate) struct RuntimeConfig {
    /// Default seconds to wait for a host to come online after sending `WoL` packets.
    /// Can be overridden per host with `wake_timeout_secs`.
    pub default_wake_timeout_secs: u64,
    /// Default seconds to wait for a host to go offline after sending a shutdown command.
    /// Can be overridden per host with `shutdown_timeout_secs`.
    pub default_shutdown_timeout_secs: u64,
    /// Interval in seconds between background host-status poll cycles.
    pub status_poll_interval_secs: u64,
    /// Interval in milliseconds between state checks during a wake/shutdown transition.
    pub transition_poll_interval_ms: u64,
    /// Seconds a diverged enforced-host state must be stable before the enforcer
    /// re-triggers a wake / shutdown (prevents hammering during transitions).
    pub enforce_stabilization_threshold_secs: u64,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            default_wake_timeout_secs: 120,
            default_shutdown_timeout_secs: 20,
            status_poll_interval_secs: 2,
            transition_poll_interval_ms: 200,
            enforce_stabilization_threshold_secs: 5,
        }
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
    /// Runtime tuning parameters (poll intervals, default timeouts, etc.).
    pub runtime: RuntimeConfig,
    /// When `false`, disables the periodic GitHub release check. Defaults to `true`.
    pub check_for_updates: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: 8080,
            bind: "127.0.0.1".to_string(),
            broadcast_port: shuthost_common::DEFAULT_COORDINATOR_BROADCAST_PORT,
            tls: None,
            auth: AuthConfig::default(),
            runtime: RuntimeConfig::default(),
            check_for_updates: true,
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
    /// External auth was configured (reverse proxy / external provider).
    External {
        /// Records which set/level of exceptions the operator acknowledged;
        /// the UI will show a warning when this doesn't match the current
        /// expected version so operators can update their proxy rules.
        exceptions_version: u32,
    },
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

/// A simple (string) event filter that matches all hosts for that event type.
#[derive(Debug, Deserialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SimpleEventFilter {
    Unscheduled,
    OperationFailed,
}

pub(crate) type Hosts = Option<Vec<String>>;

/// A structured (table) event filter, allowing host scoping and carrying
/// extra data for event types that need it (e.g. `OnlineFor`).
#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum StructuredEventFilter {
    Unscheduled {
        #[serde(default)]
        hosts: Hosts,
    },
    OperationFailed {
        #[serde(default)]
        hosts: Hosts,
    },
    OnlineFor {
        duration_secs: u64,
        #[serde(default)]
        hosts: Hosts,
    },
}

/// A webhook event filter — either a plain string (`"unscheduled"`) or an inline
/// table (`{ type = "online_for", duration_secs = 300 }`).
///
/// Serde tries `Simple` first (string match) and falls back to `Structured` (table).
/// Note: `#[serde(untagged)]` produces poor error messages on typos — acceptable for
/// a config file where the operator can inspect the file directly.
#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(untagged)]
pub(crate) enum WebhookEventFilter {
    /// Plain string shorthand for events that don't require extra data and match all hosts.
    Simple(SimpleEventFilter),
    /// Inline table, needed for `OnlineFor` or to scope by host.
    Structured(StructuredEventFilter),
}

/// Configuration for a single webhook endpoint.
#[derive(Debug, Deserialize, Clone)]
pub(crate) struct WebhookConfig {
    /// The URL to POST notifications to.
    pub url: String,
    /// Which events to fire for.
    ///
    /// - Absent (`None`): fires for `unscheduled` and `operation_failed` events for all hosts.
    ///   `online_for` is never included by default — it must be listed explicitly.
    /// - Empty list: fires for nothing (effectively disables the webhook).
    /// - Non-empty list: fires only for the listed filters.
    #[serde(default)]
    pub events: Option<Vec<WebhookEventFilter>>,
    /// Optional extra HTTP headers (e.g. `Authorization = "Bearer token"`).
    #[serde(default)]
    pub headers: HashMap<String, Arc<SecretString>>,
    /// Optional shared secret. When set, each POST includes an
    /// `X-ShutHost-Signature: sha256=<hex>` header containing the HMAC-SHA256
    /// of the raw JSON body, signed with this secret.
    pub secret: Option<Arc<SecretString>>,
}

impl PartialEq for WebhookConfig {
    fn eq(&self, other: &Self) -> bool {
        self.url == other.url
            && self.events == other.events
            && self.headers.len() == other.headers.len()
            && self.headers.iter().all(|(k, v)| {
                other
                    .headers
                    .get(k)
                    .is_some_and(|ov| ov.expose_secret() == v.expose_secret())
            })
            && match (self.secret.as_ref(), other.secret.as_ref()) {
                (Some(a), Some(b)) => a.expose_secret() == b.expose_secret(),
                (None, None) => true,
                _ => false,
            }
    }
}

/// Top-level notifications configuration block.
#[derive(Debug, Deserialize, Clone, PartialEq, Default)]
#[serde(default)]
pub(crate) struct NotificationsConfig {
    /// List of webhook endpoints to fire on notification events.
    pub webhooks: Vec<WebhookConfig>,
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
    /// Notification delivery configuration (webhooks, etc.).
    #[serde(default)]
    pub notifications: NotificationsConfig,
}
