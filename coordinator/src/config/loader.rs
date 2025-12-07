//! Configuration loading utilities for the coordinator.
//!
//! This module provides functions for reading and parsing
//! configuration files from disk.

use std::path::Path;

use eyre::WrapErr;

use crate::config::ControllerConfig;

/// Reads and parses the coordinator config from a TOML file.
///
/// # Arguments
///
/// * `path` - File path to the TOML configuration file.
///
/// # Errors
///
/// Returns an error if the config file cannot be read or parsed.
pub(crate) async fn load<P: AsRef<Path>>(path: P) -> eyre::Result<ControllerConfig> {
    let path_ref = path.as_ref();
    let content = tokio::fs::read_to_string(&path).await.wrap_err(format!(
        "Failed to read config file at: {}",
        path_ref.display()
    ))?;
    let config: ControllerConfig = toml::from_str(&content).wrap_err(format!(
        "Failed to parse config as TOML at: {}",
        path_ref.display()
    ))?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use crate::config::AuthMode;
    use std::fs;
    use std::process::Command;

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
        let cfg = load(&tmp).await.unwrap();
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

    #[tokio::test]
    async fn test_load_coordinator_config_missing_file() {
        let tmp = std::env::temp_dir().join("does_not_exist.toml");
        let res = load(&tmp).await;
        assert!(res.is_err(), "Expected error for missing file");
    }

    #[tokio::test]
    async fn test_load_coordinator_config_invalid_toml() {
        let tmp = std::env::temp_dir().join("invalid.toml");
        std::fs::write(&tmp, "not valid toml").unwrap();
        let res = load(&tmp).await;
        assert!(res.is_err(), "Expected error for invalid TOML");
    }

    #[tokio::test]
    async fn test_tls_absent_field_results_in_none() {
        let toml_str = r#"
            [server]
            port = 8081
            bind = "127.0.0.1"

            [hosts]

            [clients]
        "#;
        let tmp = std::env::temp_dir().join("test_config_no_tls.toml");
        std::fs::write(&tmp, toml_str).unwrap();
        let cfg = load(&tmp).await.unwrap();
        assert!(
            cfg.server.tls.is_none(),
            "Expected tls to be None when omitted"
        );
    }

    #[tokio::test]
    async fn test_tls_empty_table_uses_defaults() {
        let toml_str = r#"
            [server]
            port = 8082
            bind = "0.0.0.0"

            [server.tls]

            [hosts]

            [clients]
        "#;
        let tmp = std::env::temp_dir().join("test_config_tls_defaults.toml");
        std::fs::write(&tmp, toml_str).unwrap();
        let cfg = load(&tmp).await.unwrap();
        let tls = cfg
            .server
            .tls
            .expect("tls should be present when table exists");
        assert_eq!(tls.cert_path, "./tls_cert.pem");
        assert_eq!(tls.key_path, "./tls_key.pem");
        assert!(
            tls.persist_self_signed,
            "persist_self_signed should default to true"
        );
    }

    #[test]
    fn test_tls_custom_values_deserialize() {
        let toml_str = r#"
            [server]
            port = 8083
            bind = "::1"

            [server.tls]
            cert_path = "certs/mycert.pem"
            key_path = "certs/mykey.pem"
            persist_self_signed = false

            [hosts]

            [clients]
        "#;
        let cfg: crate::config::ControllerConfig =
            toml::from_str(toml_str).expect("Failed to parse TOML");
        let tls = cfg.server.tls.expect("tls should be present");
        assert_eq!(tls.cert_path, "certs/mycert.pem");
        assert_eq!(tls.key_path, "certs/mykey.pem");
        assert!(!tls.persist_self_signed);
    }

    #[tokio::test]
    async fn test_load_example_config() {
        let temp_file = std::env::temp_dir().join("test_example_config.toml");
        fs::copy("../docs/examples/example_config.toml", &temp_file).unwrap();
        let cfg = load(&temp_file)
            .await
            .expect("Failed to load example_config.toml");
        assert_eq!(cfg.server.port, 8080);
        assert_eq!(cfg.server.bind, "127.0.0.1");
        assert_eq!(cfg.db, Some(Default::default()));
        assert!(matches!(cfg.server.auth.mode, AuthMode::Token { .. }));
    }

    #[tokio::test]
    async fn test_load_example_config_with_client_and_host() {
        let temp_file = std::env::temp_dir().join("test_example_config_with_client_and_host.toml");
        fs::copy("../docs/examples/example_config.toml", &temp_file).unwrap();
        Command::new("patch")
            .arg("-i")
            .arg("../docs/examples/example_config_with_client_and_host.toml.patch")
            .arg(&temp_file)
            .status()
            .unwrap();
        let cfg = load(&temp_file)
            .await
            .expect("Failed to load example_config_with_client_and_host.toml");
        assert!(cfg.hosts.contains_key("my-host-name"));
        assert!(cfg.clients.contains_key("my-client-name"));
    }

    #[tokio::test]
    async fn test_load_example_config_external() {
        let temp_file = std::env::temp_dir().join("test_example_config_external.toml");
        fs::copy("../docs/examples/example_config.toml", &temp_file).unwrap();
        Command::new("patch")
            .arg("-i")
            .arg("../docs/examples/example_config_external.toml.patch")
            .arg(&temp_file)
            .status()
            .unwrap();
        let cfg = load(&temp_file)
            .await
            .expect("Failed to load example_config_external.toml");
        assert_eq!(
            cfg.server.auth.mode,
            AuthMode::External {
                exceptions_version: 0
            }
        );
    }

    #[tokio::test]
    async fn test_load_example_config_oidc() {
        let temp_file = std::env::temp_dir().join("test_example_config_oidc.toml");
        fs::copy("../docs/examples/example_config.toml", &temp_file).unwrap();
        Command::new("patch")
            .arg("-i")
            .arg("../docs/examples/example_config_oidc.toml.patch")
            .arg(&temp_file)
            .status()
            .unwrap();
        let cfg = load(&temp_file)
            .await
            .expect("Failed to load example_config_oidc.toml");
        assert_eq!(
            cfg.server.auth.mode,
            AuthMode::Oidc {
                issuer: "https://your-oidc-provider.com/realms/your-realm".to_string(),
                client_id: "shuthost".to_string(),
                client_secret: "your-client-secret".to_string(),
                scopes: vec!["openid".to_string(), "profile".to_string()]
            }
        );
    }
}
