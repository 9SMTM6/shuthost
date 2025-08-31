//! Coordinator installer: sets up the service to run the web control interface on boot.
//!
//! Supports systemd, OpenRC, and launchd based on target OS.

use clap::Parser;
#[cfg(target_os = "linux")]
use shuthost_common::{is_openrc, is_systemd};
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
const OPENRC_FILE_TEMPLATE: &str = include_str!("openrc.shuthost_coordinator.sh");

/// Arguments for the `install` subcommand of the coordinator.
#[derive(Debug, Parser)]
pub struct InstallArgs {
    /// Username to own the generated config file (from $SUDO_USER).
    #[arg(env = "SUDO_USER")]
    user: String,

    /// Port on which the coordinator HTTP server will listen.
    #[arg(long = "port", default_value_t = 8080)]
    port: u16,

    /// Bind address for the HTTP server (e.g., 127.0.0.1 or 0.0.0.0).
    #[arg(long = "bind", default_value = "127.0.0.1")]
    bind: String,
}

/// Installs the coordinator as a system service and creates its config file.
///
/// # Arguments
///
/// * `args` - Installation arguments including user, port, and bind address.
///
/// # Errors
///
/// Returns `Err` if any filesystem, templating, or service management step fails.
pub fn install_coordinator(args: InstallArgs) -> Result<(), String> {
    let name = env!("CARGO_PKG_NAME");
    let user = args.user;

    args.bind
        .parse::<IpAddr>()
        .map_err(|e| format!("Invalid bind address: {e}"))?;

    // sadly, due to the installation running under sudo, I can't use $XDG_CONFIG_HOME
    #[cfg(target_os = "linux")]
    let config_location = PathBuf::from(format!("/home/{user}/.config/{name}.toml",));
    #[cfg(target_os = "macos")]
    let config_location = PathBuf::from(format!("/Users/{user}/.config/{name}.toml",));

    let bind_known_vals = |arg: &str| {
        arg.to_owned()
            .replace("{description}", env!("CARGO_PKG_DESCRIPTION"))
            .replace("{user}", &user)
            .replace("{name}", name)
            .replace("{config_location}", &config_location.to_string_lossy())
    };

    #[cfg(target_os = "linux")]
    if is_systemd() {
        shuthost_common::systemd::install_self_as_service(
            name,
            &bind_known_vals(SERVICE_FILE_TEMPLATE),
        )?;
    } else if is_openrc() {
        shuthost_common::openrc::install_self_as_service(
            name,
            &bind_known_vals(OPENRC_FILE_TEMPLATE),
        )?;
    } else {
        Err("Unsupported init system: expected systemd, OpenRC or sysvinit style.".to_string())?;
    }

    #[cfg(target_os = "macos")]
    shuthost_common::macos::install_self_as_service(name, &bind_known_vals(SERVICE_FILE_TEMPLATE))?;

    if !Path::new(&config_location).exists() {
        if let Some(parent_dir) = config_location.parent()
            && !parent_dir.exists()
        {
            std::fs::create_dir_all(parent_dir).map_err(|e| e.to_string())?;
        }

        let mut config_file = File::create(&config_location).map_err(|e| e.to_string())?;
        config_file
            .write_all(
                toml::to_string(&ControllerConfig {
                    server: ServerConfig {
                        port: args.port,
                        bind: args.bind,
                        auth: None, // TODO: change
                    },
                    ..Default::default()
                })
                .unwrap()
                .as_bytes(),
            )
            .map_err(|e| e.to_string())?;

        println!("Created config file at {config_location:?}");

        let status = Command::new("chown")
            .arg(format!("{user}:")) // ":" = default group
            .arg(&config_location)
            .status()
            .map_err(|e| e.to_string())?;

        if !status.success() {
            return Err(format!("Failed to chown file: {status}"));
        }

        println!("Chowned config file at {config_location:?} for {user}",);
    } else {
        println!("Config file already exists at {config_location:?}, not overwriting.");
    }

    #[cfg(target_os = "macos")]
    shuthost_common::macos::start_and_enable_self_as_service(name)?;

    #[cfg(target_os = "linux")]
    if is_systemd() {
        shuthost_common::systemd::start_and_enable_self_as_service(name)?;
    } else if is_openrc() {
        shuthost_common::openrc::start_and_enable_self_as_service(name)?;
    } else {
        Err("Unsupported init system: expected systemd, OpenRC or sysvinit style.".to_string())?;
    }

    Ok(())
}
