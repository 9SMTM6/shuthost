use clap::Parser;

use crate::install::{
    BINARY_NAME, InitSystem, get_default_interface, get_hostname, get_inferred_init_system, get_ip,
    get_mac,
};

const CONFIG_ENTRY: &str =
    r#""{name}" = { ip = "{ip}", mac = "{mac}", port = {port}, shared_secret = "{secret}" }"#;

#[derive(Debug, Parser)]
pub struct Args {
    /// The init system used by the host_agent installation.
    /// The service files will be parsed to extract the registration configuration.
    #[arg(long = "init-system", default_value_t = get_inferred_init_system())]
    pub init_system: InitSystem,

    /// Path to the serviceless script, required if init-system is `serviceless`.
    #[arg(long = "script-path")]
    pub script_path: Option<String>,
}

#[derive(Debug)]
pub(super) struct ServiceConfig {
    pub secret: String,
    pub port: u16,
}

pub(crate) fn parse_config_and_print_registration(
    &Args {
        init_system,
        ref script_path,
    }: &Args,
) -> Result<(), String> {
    let custom_path = if init_system == InitSystem::Serviceless {
        script_path.as_deref().ok_or_else(|| {
            "Script path must be specified for serviceless init system".to_string()
        })?
    } else {
        if script_path.is_some() {
            return Err("Script path is only valid for serviceless init system".to_string());
        }
        ""
    };

    let config = match init_system {
        #[cfg(target_os = "linux")]
        InitSystem::Systemd => parse_systemd_config()?,
        #[cfg(target_os = "linux")]
        InitSystem::OpenRC => parse_openrc_config()?,
        InitSystem::Serviceless => parse_serviceless_config(custom_path)?,
        #[cfg(target_os = "macos")]
        InitSystem::Launchd => parse_launchd_config()?,
    };

    print_registration_config(&config)?;

    Ok(())
}

pub(super) fn print_registration_config(config: &ServiceConfig) -> Result<(), String> {
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

    Ok(())
}

fn parse_systemd_config() -> Result<ServiceConfig, String> {
    let path = format!("/etc/systemd/system/{}.service", BINARY_NAME);
    let content =
        std::fs::read_to_string(&path).map_err(|e| format!("Failed to read {}: {}", path, e))?;

    let mut secret = None;
    let mut port = None;

    for line in content.lines() {
        if line.starts_with("Environment=SHUTHOST_SHARED_SECRET=") {
            secret = Some(line.split('=').nth(1).unwrap_or("").to_string());
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
fn parse_openrc_config() -> Result<ServiceConfig, String> {
    let path = format!("/etc/init.d/{}", BINARY_NAME);
    let content =
        std::fs::read_to_string(&path).map_err(|e| format!("Failed to read {}: {}", path, e))?;

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

fn parse_serviceless_config(path: &str) -> Result<ServiceConfig, String> {
    let content =
        std::fs::read_to_string(path).map_err(|e| format!("Failed to read {}: {}", path, e))?;

    let mut secret = None;
    let mut port = None;

    for line in content.lines() {
        if line.starts_with("SHUTHOST_SHARED_SECRET=") {
            secret = Some(line.split('=').nth(1).unwrap_or("").to_string());
        }
        if line.starts_with("PORT=") {
            port = Some(
                line.split('=')
                    .nth(1)
                    .unwrap_or("")
                    .parse()
                    .map_err(|_| "Invalid port".to_string())?,
            );
        }
    }

    match (secret, port) {
        (Some(s), Some(p)) => Ok(ServiceConfig { secret: s, port: p }),
        _ => Err("Failed to parse secret and port from serviceless script".to_string()),
    }
}

#[cfg(target_os = "macos")]
fn parse_launchd_config() -> Result<ServiceConfig, String> {
    let path = format!(
        "/Library/LaunchDaemons/com.github_9smtm6.{}.plist",
        BINARY_NAME
    );
    let content =
        std::fs::read_to_string(&path).map_err(|e| format!("Failed to read {}: {}", path, e))?;

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
