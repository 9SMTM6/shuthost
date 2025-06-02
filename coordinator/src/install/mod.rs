use clap::Parser;
#[cfg(target_os = "linux")]
use shuthost_common::{is_openrc, is_systemd, is_sysvinit};
#[allow(unused_imports)]
use std::os::unix::fs::PermissionsExt;
use std::{
    fs::File,
    io::Write,
    net::IpAddr,
    path::{Path, PathBuf},
    process::Command,
};

use crate::config::{ControllerConfig, ServerConfig};

#[cfg(target_os = "linux")]
const SERVICE_FILE_TEMPLATE: &str = include_str!("shuthost_coordinator.service.ini");
#[cfg(target_os = "macos")]
const SERVICE_FILE_TEMPLATE: &str =
    include_str!("com.github_9smtm6.shuthost_coordinator.plist.xml");
#[cfg(target_os = "linux")]
const SYSVINIT_INIT_TEMPLATE: &str = include_str!("sysvinit.shuthost_coordinator.sh");
#[cfg(target_os = "linux")]
const OPENRC_FILE_TEMPLATE: &str = include_str!("openrc.shuthost_coordinator.sh");

/// Struct for the install subcommand
#[derive(Debug, Parser)]
pub struct InstallArgs {
    /// Can be inferred from `$SUDO_USER` on Linux. MacOS doesn't support that, other detection mechanisms are flaky.
    #[arg(env = "SUDO_USER")]
    user: String,

    #[arg(long = "port", default_value_t = 8080)]
    port: u16,

    /// IP address to bind the HTTP server to (e.g. 127.0.0.1 / ::1 - ipv4 / ipv6 on only localhost -, 0.0.0.0 / :: - ipv4 / ipv6 on all interfaces)
    #[arg(long = "bind", default_value = "127.0.0.1")]
    bind: String,
}

pub fn install_coordinator(args: InstallArgs) -> Result<(), String> {
    let name = env!("CARGO_PKG_NAME");

    args.bind
        .parse::<IpAddr>()
        .map_err(|e| format!("Invalid bind address: {}", e))?;

    // sadly, due to the installation running under sudo, I can't use $XDG_CONFIG_HOME
    #[cfg(target_os = "linux")]
    let config_location = PathBuf::from(format!(
        "/home/{user}/.config/{name}.toml",
        user = &args.user
    ));
    #[cfg(target_os = "macos")]
    let config_location = PathBuf::from(format!(
        "/Users/{user}/.config/{name}.toml",
        user = &args.user
    ));

    let bind_known_vals = |arg: &str| {
        arg.to_owned()
            .replace("{description}", env!("CARGO_PKG_DESCRIPTION"))
            .replace("{user}", &args.user)
            .replace("{name}", name)
            .replace("{config_location}", &config_location.to_string_lossy())
    };

    #[cfg(target_os = "linux")]
    if is_systemd() {
        shuthost_common::install_self_as_service_systemd(
            &name,
            &bind_known_vals(SERVICE_FILE_TEMPLATE),
        )?;
    } else if is_openrc() {
        shuthost_common::install_self_as_service_openrc_linux(
            &name,
            &bind_known_vals(OPENRC_FILE_TEMPLATE),
        )?;
    } else if is_sysvinit() {
        shuthost_common::install_self_as_service_sysvinit_linux(
            &name,
            &bind_known_vals(SYSVINIT_INIT_TEMPLATE),
        )?;
    } else {
        Err("Unsupported init system: expected systemd, OpenRC or sysvinit style.".to_string())?;
    }

    #[cfg(target_os = "macos")]
    shuthost_common::install_self_as_service_macos(name, &bind_known_vals(SERVICE_FILE_TEMPLATE))?;

    if !Path::new(&config_location).exists() {
        let mut config_file = File::create(&config_location).map_err(|e| e.to_string())?;
        config_file
            .write_all(
                toml::to_string(&ControllerConfig {
                    server: ServerConfig {
                        port: args.port,
                        bind: args.bind,
                    },
                    ..Default::default()
                })
                .unwrap()
                .as_bytes(),
            )
            .map_err(|e| e.to_string())?;

        println!("Created config file at {config_location:?}");

        let status = Command::new("chown")
            .arg(format!("{}:", &args.user)) // ":" = default group
            .arg(&config_location)
            .status()
            .map_err(|e| e.to_string())?;

        if !status.success() {
            return Err(format!("Failed to chown file: {}", status));
        }

        println!(
            "Chowned config file at {config_location:?} for {}",
            args.user
        );
    } else {
        println!("Config file already exists at {config_location:?}, not overwriting.");
    }

    #[cfg(target_os = "macos")]
    shuthost_common::start_and_enable_self_as_service_macos(name)?;

    #[cfg(target_os = "linux")]
    if is_systemd() {
        shuthost_common::start_and_enable_self_as_service_systemd(
            &name,
        )?;
    } else if is_openrc() {
        shuthost_common::start_and_enable_self_as_service_openrc_linux(
            &name,
        )?;
    } else if is_sysvinit() {
        shuthost_common::start_and_enable_self_as_service_sysvinit_linux(
            &name,
        )?;
    } else {
        Err("Unsupported init system: expected systemd, OpenRC or sysvinit style.".to_string())?;
    }

    Ok(())
}
