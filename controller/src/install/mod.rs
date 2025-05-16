use clap::Parser;
#[cfg(target_os = "linux")]
use global_service_install::{is_systemd, is_openrc, is_sysvinit};
#[allow(unused_imports)]
use std::os::unix::fs::PermissionsExt;
use std::{collections::HashMap, fs::File, io::Write, path::PathBuf, process::Command};

use crate::config::{ControllerConfig, ServerConfig};

#[cfg(target_os = "linux")]
const SERVICE_FILE_TEMPLATE: &str = include_str!("shuthost_controller.service.ini");
#[cfg(target_os = "macos")]
const SERVICE_FILE_TEMPLATE: &str = include_str!("com.github_9smtm6.shuthost_controller.plist.xml");
#[cfg(target_os = "linux")]
const SYSVINIT_INIT_TEMPLATE: &str = include_str!("sysvinit.shuthost_controller.sh");
#[cfg(target_os = "linux")]
const OPENRC_FILE_TEMPLATE: &str = include_str!("openrc.shuthost_controller.sh");

/// Struct for the install subcommand
#[derive(Debug, Parser)]
pub struct InstallArgs {
    /// Can be inferred from `$SUDO_USER` on Linux. MacOS doesn't support that, other detection mechanisms are flaky.
    #[arg(env = "SUDO_USER")]
    user: String,
    #[arg(long = "port", default_value_t = 8080)]
    port: u16,
}

pub fn install_controller(args: InstallArgs) -> Result<(), String> {
    let name = env!("CARGO_PKG_NAME");

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
        global_service_install::install_self_as_service_systemd(
            &name,
            &bind_known_vals(SERVICE_FILE_TEMPLATE),
        )?;
    } else if is_openrc() {
        global_service_install::install_self_as_service_openrc_linux(
            &name,
            &bind_known_vals(OPENRC_FILE_TEMPLATE),
        )?;
    } else if is_sysvinit() {
        global_service_install::install_self_as_service_sysvinit_linux(
            &name,
            &bind_known_vals(SYSVINIT_INIT_TEMPLATE),
        )?;
    } else {
        Err("Unsupported init system: expected systemd, OpenRC or sysvinit style.".to_string())?;
    }

    #[cfg(target_os = "macos")]
    global_service_install::install_self_as_service_macos(
        name,
        &bind_known_vals(SERVICE_FILE_TEMPLATE),
    )?;

    let mut config_file = File::create(&config_location).map_err(|e| e.to_string())?;
    config_file
        .write_all(
            toml::to_string(&ControllerConfig {
                hosts: HashMap::new(),
                server: ServerConfig { port: args.port },
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

    println!(
        "Chowned config file at {config_location:?} for {}",
        args.user
    );

    if status.success() {
        Ok(())
    } else {
        Err(format!("chown failed: exit status {}", status))
    }
}
