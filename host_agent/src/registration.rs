use clap::Parser;

use crate::install::{
    BINARY_NAME, InitSystem, get_default_interface, get_hostname, get_inferred_init_system, get_ip,
    get_mac,
};
use shuthost_common::{ResultMapErrExt as _, UnwrapToStringExt as _};

const CONFIG_ENTRY: &str =
    r#""{name}" = { ip = "{ip}", mac = "{mac}", port = {port}, shared_secret = "{secret}" }"#;

/// Generic function to parse service config from a service name using path getter and content parser.
fn parse_config_from_path(
    get_path_fn: fn(&str) -> String,
    parse_content_fn: fn(&str) -> Result<ServiceConfig, String>,
) -> Result<ServiceConfig, String> {
    let path = get_path_fn(BINARY_NAME);
    let content =
        std::fs::read_to_string(&path).map_err_to_string(&format!("Failed to read {path}"))?;
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
            .replace("{name}", &get_hostname().expect("failed to get hostname"))
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
}

#[cfg(any(target_os = "linux", test))]
fn parse_systemd_content(content: &str) -> Result<ServiceConfig, String> {
    let mut secret = None;
    let mut port = None;

    for line in content.lines() {
        if let Some(value) = line.strip_prefix("Environment=SHUTHOST_SHARED_SECRET=") {
            secret = Some(value.to_string());
        }
        if line.contains(" --port=")
            && let Some(start) = line.find(" --port=")
        {
            let after = &line[start + 8..];
            if let Some(end) = after.find(' ') {
                port = after[..end].parse().ok();
            } else {
                port = after.parse().ok();
            }
        }
    }

    match (secret, port) {
        (Some(s), Some(p)) => Ok(ServiceConfig { secret: s, port: p }),
        _ => Err("Failed to parse secret and port from systemd service file".to_string()),
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
        if line.contains(" --port=")
            && let Some(start) = line.find(" --port=")
        {
            let after = &line[start + 8..];
            if let Some(end) = after.find(' ') {
                port = after[..end].trim_matches('"').parse().ok();
            } else {
                port = after.trim_matches('"').parse().ok();
            }
        }
    }

    match (secret, port) {
        (Some(s), Some(p)) => Ok(ServiceConfig { secret: s, port: p }),
        _ => Err("Failed to parse secret and port from openrc service file".to_string()),
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
    let Some(port) = content.lines().find_map(|line| {
        let s = line
            .strip_prefix("export PORT=\"")
            .and_then(|s| s.strip_suffix("\""))?;
        s.parse().ok()
    }) else {
        return Err("PORT not found in self-extracting script".to_string());
    };

    Ok(ServiceConfig {
        secret: secret.to_string(),
        port,
    })
}

#[cfg(unix)]
fn parse_self_extracting_shell_config(path: &str) -> Result<ServiceConfig, String> {
    let content =
        std::fs::read_to_string(path).map_err_to_string(&format!("Failed to read {path}"))?;

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
    let Some(port) = content.lines().find_map(|line| {
        let s = line
            .strip_prefix("$env:PORT = \"")
            .and_then(|s| s.strip_suffix("\""))?;
        s.parse().ok()
    }) else {
        return Err("PORT not found in self-extracting PowerShell script".to_string());
    };

    Ok(ServiceConfig {
        secret: secret.to_string(),
        port,
    })
}

fn parse_self_extracting_pwsh_config(path: &str) -> Result<ServiceConfig, String> {
    let content =
        std::fs::read_to_string(path).map_err_to_string(&format!("Failed to read {path}"))?;

    parse_self_extracting_pwsh_content(&content)
}

#[cfg(any(target_os = "macos", test))]
fn parse_launchd_content(content: &str) -> Result<ServiceConfig, String> {
    let mut secret = None;
    let mut port = None;
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
        if line.starts_with("<string>--port=") && line.ends_with("</string>") {
            let val = &line[15..line.len() - 9];
            port = val.parse().ok();
        }
    }

    match (secret, port) {
        (Some(s), Some(p)) => Ok(ServiceConfig { secret: s, port: p }),
        _ => Err("Failed to parse secret and port from launchd plist file".to_string()),
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
        let content =
            install::bind_template_replacements(template, "test desc", port, "test cmd", secret);

        let config = parse_fn(&content).unwrap();
        assert_eq!(config.secret, secret);
        assert_eq!(config.port, port);
    }

    #[test]
    fn test_parse_systemd_content() {
        test_parse_content(
            install::SYSTEMD_SERVICE_FILE_TEMPLATE,
            parse_systemd_content,
        );
    }

    #[test]
    fn test_parse_openrc_content() {
        test_parse_content(install::OPENRC_SERVICE_FILE_TEMPLATE, parse_openrc_content);
    }

    #[test]
    fn test_parse_launchd_content() {
        test_parse_content(
            install::LAUNCHD_SERVICE_FILE_TEMPLATE,
            parse_launchd_content,
        );
    }

    #[test]
    fn test_parse_self_extracting_shell_content() {
        test_parse_content(
            install::SELF_EXTRACTING_SHELL_TEMPLATE,
            parse_self_extracting_shell_content,
        );
    }

    #[test]
    fn test_parse_self_extracting_pwsh_content() {
        test_parse_content(
            install::SELF_EXTRACTING_PWSH_TEMPLATE,
            parse_self_extracting_pwsh_content,
        );
    }
}
