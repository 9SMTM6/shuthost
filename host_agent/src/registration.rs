use std::fs;

use clap::Parser;

use crate::install::{
    BINARY_NAME, InitSystem, get_default_interface, get_inferred_init_system, get_ip, get_mac,
};
use shuthost_common::{ResultMapErrExt as _, UnwrapToStringExt as _};

const CONFIG_ENTRY: &str =
    r#""{name}" = { ip = "{ip}", mac = "{mac}", port = {port}, shared_secret = "{secret}" }"#;

/// Helper function to find and extract flag values from service file lines.
///
/// # Arguments
/// * `line` - The line to search in
/// * `flag` - The flag name (without --)
/// * `delimiter` - The delimiter string to stop at
///
/// Returns the extracted value with quotes trimmed, or None if not found.
fn find_flag_value(line: &str, flag: &str, delimiter: &str) -> Option<String> {
    let pattern = format!("--{flag}=");
    line.find(&pattern).map(|start| {
        let value_slice = &line[start + pattern.len()..];
        let value = if let Some(end) = value_slice.find(delimiter) {
            &value_slice[..end]
        } else {
            value_slice
        };
        value.trim_matches('"').to_string()
    })
}

/// Generic function to parse service config from a service name using path getter and content parser.
fn parse_config_from_path(
    get_path_fn: fn(&str) -> String,
    parse_content_fn: fn(&str) -> Result<ServiceConfig, String>,
) -> Result<ServiceConfig, String> {
    let path = get_path_fn(BINARY_NAME);
    let content = fs::read_to_string(&path).map_err_to_string(&format!("Failed to read {path}"))?;
    parse_content_fn(&content)
}

#[derive(Debug, Parser)]
pub struct Args {
    /// The init system used by the `host_agent` installation.
    /// The service files will be parsed to extract the registration configuration.
    #[arg(long, short, default_value_t = get_inferred_init_system())]
    pub init_system: InitSystem,

    /// Path to the self-extracting script, only used if init-system is `self-extracting-*`.
    #[arg(long, short = 'p')]
    pub script_path: Option<String>,
}

#[derive(Debug)]
pub(crate) struct ServiceConfig {
    pub secret: String,
    pub port: u16,
    pub broadcast_port: u16,
    pub hostname: String,
}

pub(crate) fn parse_config(args: &Args) -> Result<ServiceConfig, String> {
    let custom_path = match args.init_system {
        InitSystem::SelfExtractingPwsh => args
            .script_path
            .clone()
            .unwrap_or_to_string(&format!("{BINARY_NAME}_self_extracting.ps1")),
        #[cfg(unix)]
        InitSystem::SelfExtractingShell => args
            .script_path
            .clone()
            .unwrap_or_to_string(&format!("{BINARY_NAME}_self_extracting.sh")),
        _ => {
            if args.script_path.is_some() {
                return Err("Script path is only valid for SelfExtracting* init system".to_string());
            }
            String::new()
        }
    };

    Ok(match args.init_system {
        #[cfg(target_os = "linux")]
        InitSystem::Systemd => parse_systemd_config()?,
        #[cfg(target_os = "linux")]
        InitSystem::OpenRC => parse_openrc_config()?,
        #[cfg(unix)]
        InitSystem::SelfExtractingShell => parse_self_extracting_shell_config(&custom_path)?,
        InitSystem::SelfExtractingPwsh => parse_self_extracting_pwsh_config(&custom_path)?,
        #[cfg(target_os = "macos")]
        InitSystem::Launchd => parse_launchd_config()?,
    })
}

pub(crate) fn print_registration_config(config: &ServiceConfig) {
    let interface = &get_default_interface();
    if interface.is_none() {
        eprintln!(
            "Failed to determine the default network interface. Continuing on assuming docker or similar environment."
        );
    }
    println!(
        "Place the following in the coordinator:\n{config_entry}",
        config_entry = CONFIG_ENTRY
            .replace("{name}", &config.hostname)
            .replace(
                "{ip}",
                &interface
                    .as_ref()
                    .and_then(|it| get_ip(it))
                    .unwrap_or("unrecognized".to_string())
            )
            .replace(
                "{mac}",
                &interface
                    .as_ref()
                    .and_then(|it| get_mac(it))
                    .unwrap_or("unrecognized".to_string())
            )
            .replace("{port}", &config.port.to_string())
            .replace("{secret}", &config.secret)
    );
    println!(
        "Ensure the coordinator sets `broadcast_port` to {} for this host (defaults to {}).",
        config.broadcast_port,
        shuthost_common::DEFAULT_COORDINATOR_BROADCAST_PORT
    );
}

#[cfg(any(target_os = "linux", test))]
fn parse_systemd_content(content: &str) -> Result<ServiceConfig, String> {
    let mut secret = None;
    let mut port = None;
    let mut broadcast_port = None;
    let mut hostname = None;

    for line in content.lines() {
        if let Some(value) = line.strip_prefix("Environment=SHUTHOST_SHARED_SECRET=") {
            secret = Some(value.to_string());
        }
        if let Some(value) = find_flag_value(line, "port", " ") {
            port = value.parse().ok();
        }
        if let Some(value) = find_flag_value(line, "broadcast-port", " ") {
            broadcast_port = value.parse().ok();
        }
        if let Some(value) = find_flag_value(line, "hostname", " ") {
            hostname = Some(value);
        }
    }

    match (secret, port, hostname) {
        (Some(s), Some(p), Some(h)) => Ok(ServiceConfig {
            secret: s,
            port: p,
            broadcast_port: broadcast_port
                .unwrap_or(shuthost_common::DEFAULT_COORDINATOR_BROADCAST_PORT),
            hostname: h,
        }),
        _ => {
            Err("Failed to parse secret, port, and hostname from systemd service file".to_string())
        }
    }
}

#[cfg(target_os = "linux")]
fn parse_systemd_config() -> Result<ServiceConfig, String> {
    parse_config_from_path(
        shuthost_common::systemd::get_service_path,
        parse_systemd_content,
    )
}

#[cfg(any(target_os = "linux", test))]
fn parse_openrc_content(content: &str) -> Result<ServiceConfig, String> {
    let mut secret = None;
    let mut port = None;
    let mut broadcast_port = None;
    let mut hostname = None;

    for line in content.lines() {
        if line.starts_with("export SHUTHOST_SHARED_SECRET=") {
            secret = Some(
                line.split('=')
                    .nth(1)
                    .unwrap_or("")
                    .trim_matches('"')
                    .to_string(),
            );
        }
        if let Some(value) = find_flag_value(line, "port", " ") {
            port = value.parse().ok();
        }
        if let Some(value) = find_flag_value(line, "broadcast-port", " ") {
            broadcast_port = value.parse().ok();
        }
        if let Some(value) = find_flag_value(line, "hostname", " ") {
            hostname = Some(value);
        }
    }

    match (secret, port, hostname) {
        (Some(s), Some(p), Some(h)) => Ok(ServiceConfig {
            secret: s,
            port: p,
            broadcast_port: broadcast_port
                .unwrap_or(shuthost_common::DEFAULT_COORDINATOR_BROADCAST_PORT),
            hostname: h,
        }),
        _ => Err("Failed to parse secret, port, and hostname from openrc service file".to_string()),
    }
}

#[cfg(target_os = "linux")]
fn parse_openrc_config() -> Result<ServiceConfig, String> {
    parse_config_from_path(
        shuthost_common::openrc::get_service_path,
        parse_openrc_content,
    )
}

#[cfg(unix)]
fn parse_self_extracting_shell_content(content: &str) -> Result<ServiceConfig, String> {
    let Some(secret) = content.lines().find_map(|line| {
        let s = line.strip_prefix("export SHUTHOST_SHARED_SECRET=\"")?;
        s.strip_suffix("\"")
    }) else {
        return Err("SHUTHOST_SHARED_SECRET not found in self-extracting script".to_string());
    };
    let Some(hostname) = content.lines().find_map(|line| {
        let s = line.strip_prefix("export SHUTHOST_HOSTNAME=\"")?;
        s.strip_suffix("\"")
    }) else {
        return Err("SHUTHOST_HOSTNAME not found in self-extracting script".to_string());
    };
    let Some(port) = content.lines().find_map(|line| {
        let s = line
            .strip_prefix("export PORT=\"")
            .and_then(|s| s.strip_suffix("\""))?;
        s.parse().ok()
    }) else {
        return Err("PORT not found in self-extracting script".to_string());
    };
    let broadcast_port = content.lines().find_map(|line| {
        let s = line
            .strip_prefix("export BROADCAST_PORT=\"")
            .and_then(|s| s.strip_suffix("\""))?;
        s.parse().ok()
    });

    Ok(ServiceConfig {
        secret: secret.to_string(),
        port,
        broadcast_port: broadcast_port
            .unwrap_or(shuthost_common::DEFAULT_COORDINATOR_BROADCAST_PORT),
        hostname: hostname.to_string(),
    })
}

#[cfg(unix)]
fn parse_self_extracting_shell_config(path: &str) -> Result<ServiceConfig, String> {
    let content = fs::read_to_string(path).map_err_to_string(&format!("Failed to read {path}"))?;

    parse_self_extracting_shell_content(&content)
}

fn parse_self_extracting_pwsh_content(content: &str) -> Result<ServiceConfig, String> {
    let Some(secret) = content.lines().find_map(|line| {
        let s = line.strip_prefix("$env:SHUTHOST_SHARED_SECRET = \"")?;
        s.strip_suffix("\"")
    }) else {
        return Err(
            "SHUTHOST_SHARED_SECRET not found in self-extracting PowerShell script".to_string(),
        );
    };
    let Some(hostname) = content.lines().find_map(|line| {
        let s = line.strip_prefix("$env:SHUTHOST_HOSTNAME = \"")?;
        s.strip_suffix("\"")
    }) else {
        return Err("SHUTHOST_HOSTNAME not found in self-extracting PowerShell script".to_string());
    };
    let Some(port) = content.lines().find_map(|line| {
        let s = line
            .strip_prefix("$env:PORT = \"")
            .and_then(|s| s.strip_suffix("\""))?;
        s.parse().ok()
    }) else {
        return Err("PORT not found in self-extracting PowerShell script".to_string());
    };
    let broadcast_port = content.lines().find_map(|line| {
        let s = line.strip_prefix("$env:BROADCAST_PORT = \"")?;
        s.strip_suffix("\"")
    });

    Ok(ServiceConfig {
        secret: secret.to_string(),
        port,
        broadcast_port: broadcast_port
            .and_then(|s| s.parse().ok())
            .unwrap_or(shuthost_common::DEFAULT_COORDINATOR_BROADCAST_PORT),
        hostname: hostname.to_string(),
    })
}

fn parse_self_extracting_pwsh_config(path: &str) -> Result<ServiceConfig, String> {
    let content = fs::read_to_string(path).map_err_to_string(&format!("Failed to read {path}"))?;

    parse_self_extracting_pwsh_content(&content)
}

#[cfg(any(target_os = "macos", test))]
fn parse_launchd_content(content: &str) -> Result<ServiceConfig, String> {
    let mut secret = None;
    let mut port = None;
    let mut broadcast_port = None;
    let mut hostname = None;
    let mut in_secret = false;

    for line in content.lines() {
        let line = line.trim();
        if line == "<key>SHUTHOST_SHARED_SECRET</key>" {
            in_secret = true;
        } else if in_secret && line.starts_with("<string>") && line.ends_with("</string>") {
            let val = &line[8..line.len() - 9];
            secret = Some(val.to_string());
            in_secret = false;
        }
        if let Some(value) = find_flag_value(line, "port", "</string>") {
            port = value.parse().ok();
        }
        if let Some(value) = find_flag_value(line, "broadcast-port", "</string>") {
            broadcast_port = value.parse().ok();
        }
        if line.contains("--hostname")
            && let Some(value) = find_flag_value(line, "hostname", "</string>")
        {
            hostname = Some(value);
        }
    }

    match (secret, port, hostname) {
        (Some(s), Some(p), Some(h)) => Ok(ServiceConfig {
            secret: s,
            port: p,
            broadcast_port: broadcast_port
                .unwrap_or(shuthost_common::DEFAULT_COORDINATOR_BROADCAST_PORT),
            hostname: h,
        }),
        _ => Err("Failed to parse secret, port, and hostname from launchd plist file".to_string()),
    }
}

#[cfg(target_os = "macos")]
fn parse_launchd_config() -> Result<ServiceConfig, String> {
    parse_config_from_path(
        shuthost_common::macos::get_service_path,
        parse_launchd_content,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install;

    fn test_parse_content(template: &str, parse_fn: fn(&str) -> Result<ServiceConfig, String>) {
        let secret = "test_secret";
        let port = 1234;
        let hostname = "test_hostname";
        let content = install::bind_template_replacements(
            template,
            "test desc",
            port,
            /* broadcast_port */ port,
            "test cmd",
            secret,
            hostname,
        );

        let config = parse_fn(&content).unwrap();
        assert_eq!(config.secret, secret);
        assert_eq!(config.port, port);
        assert_eq!(config.broadcast_port, port);
        assert_eq!(config.hostname, hostname);
        // ensure the generated template no longer contains the placeholder and that
        // the broadcast port value made it through as well.
        assert!(!content.contains("{ broadcast_port }"));
        assert!(content.contains(&port.to_string()));
    }

    #[test]
    fn parse_systemd_content_works() {
        test_parse_content(
            install::SYSTEMD_SERVICE_FILE_TEMPLATE,
            parse_systemd_content,
        );
    }

    #[test]
    fn parse_openrc_content_works() {
        test_parse_content(install::OPENRC_SERVICE_FILE_TEMPLATE, parse_openrc_content);
    }

    #[test]
    fn parse_launchd_content_works() {
        test_parse_content(
            install::LAUNCHD_SERVICE_FILE_TEMPLATE,
            parse_launchd_content,
        );
    }

    #[test]
    fn parse_self_extracting_shell_content_works() {
        test_parse_content(
            install::SELF_EXTRACTING_SHELL_TEMPLATE,
            parse_self_extracting_shell_content,
        );
    }

    #[test]
    fn parse_self_extracting_pwsh_content_works() {
        test_parse_content(
            install::SELF_EXTRACTING_PWSH_TEMPLATE,
            parse_self_extracting_pwsh_content,
        );
    }
}
