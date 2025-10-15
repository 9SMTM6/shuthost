//! Coordinator installer: sets up the service to run the web control interface on boot.
//!
//! Supports systemd, OpenRC, and launchd based on target OS.

use std::{
    fs::File,
    io::Write,
    net::IpAddr,
    path::{Path, PathBuf},
    process::Command,
};

use clap::Parser;
use eyre::WrapErr;

#[cfg(target_os = "linux")]
use shuthost_common::{is_openrc, is_systemd};

use crate::config::{AuthConfig, AuthMode, ControllerConfig, ServerConfig};

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
pub fn install_coordinator(args: InstallArgs) -> eyre::Result<()> {
    let name = env!("CARGO_PKG_NAME");
    let user = args.user;

    args.bind
        .parse::<IpAddr>()
        .wrap_err("Invalid bind address")?;

    // sadly, due to the installation running under sudo, I can't use $XDG_CONFIG_HOME
    #[cfg(target_os = "linux")]
    let config_location = PathBuf::from(format!("/home/{user}/.config/{name}.toml",));
    #[cfg(target_os = "macos")]
    let config_location = PathBuf::from(format!("/Users/{user}/.config/{name}.toml",));

    let bind_known_vals = |arg: &str| {
        arg.to_owned()
            .replace("{ description }", env!("CARGO_PKG_DESCRIPTION"))
            .replace("{ user }", &user)
            .replace("{ name }", name)
            .replace("{ config_location }", &config_location.to_string_lossy())
    };

    #[cfg(target_os = "linux")]
    if is_systemd() {
        shuthost_common::systemd::install_self_as_service(
            name,
            &bind_known_vals(SERVICE_FILE_TEMPLATE),
        )
        .map_err(eyre::Report::msg)?;
    } else if is_openrc() {
        shuthost_common::openrc::install_self_as_service(
            name,
            &bind_known_vals(OPENRC_FILE_TEMPLATE),
        )
        .map_err(eyre::Report::msg)?;
    } else {
        eyre::bail!("Unsupported init system: expected systemd, OpenRC or sysvinit style.");
    }

    #[cfg(target_os = "macos")]
    shuthost_common::macos::install_self_as_service(name, &bind_known_vals(SERVICE_FILE_TEMPLATE))
        .map_err(eyre::Report::msg)?;

    if !Path::new(&config_location).exists() {
        if let Some(parent_dir) = config_location.parent()
            && !parent_dir.exists()
        {
            std::fs::create_dir_all(parent_dir).wrap_err("Failed to create config directory")?;
        }

        let mut config_file =
            File::create(&config_location).wrap_err("Failed to create config file")?;
        config_file
            .write_all(
                toml::to_string(&ControllerConfig {
                    server: ServerConfig {
                        port: args.port,
                        bind: args.bind,
                        auth: AuthConfig {
                            mode: AuthMode::Token { token: None },
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .unwrap()
                .as_bytes(),
            )
            .wrap_err("Failed to write config file")?;

        println!("Created config file at {config_location:?}");

        let status = Command::new("chown")
            .arg(format!("{user}:")) // ":" = default group
            .arg(&config_location)
            .status()
            .wrap_err("Failed to run chown")?;

        if !status.success() {
            eyre::bail!("Failed to chown file: {status}");
        }

        println!("Chowned config file at {config_location:?} for {user}",);
    } else {
        println!("Config file already exists at {config_location:?}, not overwriting.");
    }

    #[cfg(target_os = "macos")]
    shuthost_common::macos::start_and_enable_self_as_service(name).map_err(eyre::Report::msg)?;

    #[cfg(target_os = "linux")]
    if is_systemd() {
        shuthost_common::systemd::start_and_enable_self_as_service(name)
            .map_err(eyre::Report::msg)?;
    } else if is_openrc() {
        shuthost_common::openrc::start_and_enable_self_as_service(name)
            .map_err(eyre::Report::msg)?;
    } else {
        eyre::bail!("Unsupported init system: expected systemd, OpenRC or sysvinit style.");
    }

    Ok(())
}
