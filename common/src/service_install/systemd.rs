//! Systemd service installer for Linux systems.
//!
//! Provides functions to install and enable a service unit for the current binary.

use std::{
    env,
    fs::{self, File},
    io::Write,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::{Command, Stdio},
};

use crate::{ResultMapErrExt, is_superuser};

/// Returns the systemd service file path for the given service name.
pub fn get_service_path(name: &str) -> String {
    format!("/etc/systemd/system/{name}.service")
}

/// Installs the current binary and creates a systemd service unit file.
///
/// # Arguments
///
/// * `name` - Base name for the service and binary.
/// * `init_script_content` - Template for the unit file content (`{ binary }` placeholder).
///
/// # Errors
///
/// Returns `Err` if not root or filesystem writes fail.
pub fn install_self_as_service(name: &str, init_script_content: &str) -> Result<(), String> {
    if !is_superuser() {
        return Err("You must run this command as root or with sudo.".to_string());
    }

    let binary_path = env::current_exe().map_err_to_string_simple()?;
    let target_bin = PathBuf::from("/usr/sbin/").join(name);
    let service_name = format!("{name}.service");

    // Stop potentially existing service it before overwriting
    match Command::new("systemctl")
        .arg("stop")
        .arg(&service_name)
        .stderr(Stdio::null())
        .status()
    {
        Ok(status) if status.success() => {
            println!("Stopped existing service {service_name}.");
        }
        Ok(_) => {
            println!("Service {service_name} was not running or could not be stopped.");
        }
        Err(e) => {
            return Err(format!("Failed to execute systemctl stop: {e}"));
        }
    }

    fs::copy(binary_path, &target_bin).map_err_to_string_simple()?;
    println!("Installed binary to {target_bin:?}");
    // Set binary permissions to 0755 (root can write, others can read/execute)
    fs::set_permissions(&target_bin, fs::Permissions::from_mode(0o755))
        .map_err_to_string_simple()?;
    let service_file_path = get_service_path(name);

    let mut service_file = File::create(&service_file_path).map_err_to_string_simple()?;
    service_file
        .write_all(init_script_content.as_bytes())
        .map_err_to_string_simple()?;
    // Set service file permissions to 0640 (root:root)
    fs::set_permissions(&service_file_path, fs::Permissions::from_mode(0o640))
        .map_err_to_string_simple()?;
    println!("Created systemd service file at {service_file_path}");

    drop(service_file);

    Command::new("systemctl")
        .arg("daemon-reload")
        .output()
        .map_err_to_string_simple()?;

    Ok(())
}

/// Reloads systemd, enables, and starts the service unit.
///
/// # Arguments
///
/// * `name` - Base name of the service (unit name without `.service`).
///
/// # Errors
///
/// Returns `Err` if `systemctl` commands fail.
pub fn start_and_enable_self_as_service(name: &str) -> Result<(), String> {
    let service_name = format!("{name}.service");

    Command::new("systemctl")
        .arg("daemon-reload")
        .output()
        .map_err_to_string_simple()?;

    Command::new("systemctl")
        .arg("enable")
        .arg(&service_name)
        .output()
        .map_err_to_string_simple()?;

    Command::new("systemctl")
        .arg("start")
        .arg(&service_name)
        .output()
        .map_err_to_string_simple()?;

    println!("Service {service_name} started and enabled.");
    Ok(())
}
