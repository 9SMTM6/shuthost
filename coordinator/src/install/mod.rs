//! Coordinator installer: sets up the service to run the web control interface on boot.
//!
//! Supports systemd, `OpenRC`, and launchd based on target OS.

use std::{
    fs::File,
    io::Write as _,
    net::IpAddr,
    os::unix::fs::{self, PermissionsExt as _},
    path::{Path, PathBuf},
};

use clap::Parser;
use eyre::WrapErr as _;
use nix::unistd::User;

mod migration;

#[cfg(target_os = "linux")]
use shuthost_common::{is_openrc, is_systemd};

#[cfg(target_os = "linux")]
const SERVICE_FILE_TEMPLATE: &str = include_str!("shuthost_coordinator.service.tmpl.ini");
#[cfg(target_os = "macos")]
const SERVICE_FILE_TEMPLATE: &str =
    include_str!("com.github_9smtm6.shuthost_coordinator.plist.tmpl.xml");
#[cfg(target_os = "linux")]
const OPENRC_FILE_TEMPLATE: &str = include_str!("openrc.shuthost_coordinator.tmpl.sh");

const BINARY_NAME: &str = env!("CARGO_PKG_NAME");

/// Arguments for the `install` subcommand of the coordinator.
#[derive(Debug, Parser)]
pub struct Args {
    /// Username to own the generated config file.
    #[arg(env = "SUDO_USER")]
    user: String,

    /// Port on which the coordinator HTTP server will listen.
    #[arg(long, short, default_value_t = 8080)]
    port: u16,

    /// Bind address for the HTTP server (e.g., 127.0.0.1 or 0.0.0.0).
    #[arg(long, short, default_value = "127.0.0.1")]
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
///
/// # Panics
///
/// Panics if the TOML serialization of the configuration fails.
pub(crate) fn setup(args: Args) -> eyre::Result<()> {
    let name = BINARY_NAME;
    let user = args.user;

    args.bind
        .parse::<IpAddr>()
        .wrap_err("Invalid bind address")?;

    // sadly, due to the installation running under sudo, I can't use $XDG_CONFIG_HOME
    #[cfg(target_os = "linux")]
    let new_config_location = PathBuf::from(format!("/home/{user}/.config/{name}/config.toml",));
    #[cfg(target_os = "macos")]
    let new_config_location = PathBuf::from(format!("/Users/{user}/.config/{name}/config.toml"));

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    migration::migrate_old_config(&user, &new_config_location)?;

    let config_location = new_config_location;

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

    if Path::new(&config_location).exists() {
        println!("Config file already exists at {config_location:?}, not overwriting.");
    } else {
        let created_dir = if let Some(parent_dir) = config_location.parent()
            && !parent_dir.exists()
        {
            std::fs::create_dir_all(parent_dir).wrap_err("Failed to create config directory")?;
            true
        } else {
            false
        };

        let mut config_file = File::create(&config_location).wrap_err(format!(
            "Failed to create config file at {}",
            config_location.display()
        ))?;
        let port = args.port;
        let bind = args.bind;
        let config_content = include_str!("../../../docs/examples/example_config.toml")
            .replace("port = 8080", &format!("port = {port}"))
            .replace("bind = \"127.0.0.1\"", &format!("bind = \"{bind}\""));
        config_file
            .write_all(config_content.as_bytes())
            .wrap_err("Failed to write config file")?;

        std::fs::set_permissions(&config_location, std::fs::Permissions::from_mode(0o600))?;

        println!("Created config file at {config_location:?}");
        let user_info = User::from_name(&user)
            .wrap_err("Failed to get user info")?
            .ok_or_else(|| eyre::eyre!("User {user} not found"))?;

        // Chown the config directory if it was created
        if created_dir && let Some(parent_dir) = config_location.parent() {
            std::fs::set_permissions(parent_dir, std::fs::Permissions::from_mode(0o700))?;
            fs::chown(
                parent_dir,
                Some(user_info.uid.into()),
                Some(user_info.gid.into()),
            )?;

            println!("Chowned config directory at {parent_dir:?} for {user}",);
        }

        fs::chown(
            &config_location,
            Some(user_info.uid.into()),
            Some(user_info.gid.into()),
        )?;

        println!("Chowned config file at {config_location:?} for {user}",);
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
