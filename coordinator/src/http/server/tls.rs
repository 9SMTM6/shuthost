use core::net::{IpAddr, SocketAddr};
use std::path::Path;

use axum_server::tls_rustls::RustlsConfig as AxumRustlsConfig;
use eyre::{WrapErr as _, eyre};
use secrecy::{ExposeSecret as _, SecretBox};
use tokio::fs as t_fs;

use crate::config::{TlsConfig, resolve_config_relative_paths};

/// Setup TLS configuration for HTTPS server.
///
/// Use provided certs when both files exist. Otherwise, if `persist_self_signed` is true
/// (default), generate and persist self-signed cert/key next to the config file.
#[tracing::instrument]
pub(crate) async fn setup_tls_config(
    tls_cfg: &TlsConfig,
    config_path: &Path,
    listen_ip: IpAddr,
    addr: SocketAddr,
) -> eyre::Result<AxumRustlsConfig> {
    let cert_path_cfg = tls_cfg.cert_path.as_str();
    let key_path_cfg = tls_cfg.key_path.as_str();

    let cert_path = resolve_config_relative_paths(config_path, cert_path_cfg);
    let key_path = resolve_config_relative_paths(config_path, key_path_cfg);

    let cert_exists = cert_path.exists();
    let key_exists = key_path.exists();

    let rustls_cfg = if cert_exists && key_exists {
        let rustls_cfg = AxumRustlsConfig::from_pem_file(
            cert_path
                .to_str()
                .ok_or_else(|| eyre!("Invalid Cert-Path"))?,
            key_path.to_str().ok_or_else(|| eyre!("Invalid Key-Path"))?,
        )
        .await
        .wrap_err(format!(
            "Failed to load TLS certificates from cert: {}, key: {}",
            cert_path.display(),
            key_path.display()
        ))?;
        tracing::info!("Listening on https://{} (provided certs)", addr);
        rustls_cfg
    } else if tls_cfg.persist_self_signed {
        if cert_exists ^ key_exists {
            eyre::bail!("TLS configuration error: partial cert/key files exist");
        }

        let hostnames = vec![listen_ip.to_string()];
        let rcgen::CertifiedKey { cert, signing_key } =
            rcgen::generate_simple_self_signed(hostnames)
                .wrap_err("Failed to generate self-signed certificate")?;
        let cert_pem = cert.pem();
        let key_pem = SecretBox::new(Box::new(signing_key.serialize_pem().into_bytes()));

        let cfg_dir = config_path.parent().unwrap_or_else(|| Path::new("."));
        t_fs::create_dir_all(&cfg_dir).await.wrap_err(format!(
            "Failed to create certificate directory at: {}",
            cfg_dir.display()
        ))?;

        tokio::try_join!(
            t_fs::write(&cert_path, cert_pem.as_bytes()),
            t_fs::write(&key_path, key_pem.expose_secret())
        )
        .wrap_err(format!(
            "Failed to write TLS certificates to cert: {}, key: {}",
            cert_path.display(),
            key_path.display()
        ))?;

        let rustls_cfg =
            AxumRustlsConfig::from_pem(cert_pem.into_bytes(), key_pem.expose_secret().clone())
                .await?;
        tracing::info!(
            "Listening on https://{} (self-signed, persisted at {:?})",
            addr,
            cfg_dir
        );
        rustls_cfg
    } else {
        eyre::bail!("TLS configuration error: neither provided certs nor self-signed allowed");
    };

    Ok(rustls_cfg)
}
