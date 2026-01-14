//! OpenRC service installer for Unix-like systems.
//!
//! Provides functions to install the current binary as an OpenRC init script and start it.

use std::{
    env,
    fs::{self, File},
    io::Write,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use crate::{ResultMapErrExt, is_superuser};

/// Returns the OpenRC service file path for the given service name.
pub fn get_service_path(name: &str) -> String {
    format!("/etc/init.d/{}", name)
}

/// Installs the current binary as an OpenRC service init script.
///
/// # Arguments
///
/// * `name` - Name to assign to the service and executable.
/// * `init_script_content` - Template for the OpenRC init script (with `{ binary }` placeholder).
///
/// # Errors
///
/// Returns `Err` if not running as superuser or if filesystem operations fail.
pub fn install_self_as_service(name: &str, init_script_content: &str) -> Result<(), String> {
    if !is_superuser() {
        return Err("You must run this command as root or with sudo.".to_string());
    }

    let binary_path = env::current_exe().map_err_to_string_simple()?;
    let target_bin = Path::new("/usr/sbin/").join(name);
    let init_script_path = PathBuf::from(get_service_path(name));

    // Stop and remove any existing service
    // Attempt to stop the service if it's running, but don't fail if it isn't
    match Command::new("rc-service")
        .arg(name)
        .arg("stop")
        .stderr(Stdio::null())
        .status()
    {
        Ok(status) if status.success() => {
            println!("Stopped existing service {name}.");
        }
        Ok(_) => {
            println!("Service {name} was not running or could not be stopped.");
        }
        Err(e) => {
            return Err(format!("Failed to execute rc-service stop: {e}"));
        }
    }

    fs::copy(&binary_path, &target_bin).map_err_to_string_simple()?;
    println!("Installed binary to {target_bin:?}");
    // Set binary permissions to 0755 (root can write, others can read/execute)
    fs::set_permissions(&target_bin, fs::Permissions::from_mode(0o755))
        .map_err_to_string_simple()?;

    let mut script_file = File::create(&init_script_path).map_err_to_string_simple()?;
    script_file
        .write_all(init_script_content.as_bytes())
        .map_err_to_string_simple()?;

    let mut perms = script_file
        .metadata()
        .map_err_to_string_simple()?
        .permissions();
    perms.set_mode(0o750);
    fs::set_permissions(&init_script_path, perms).map_err_to_string_simple()?;
    println!("Created OpenRC init script at {init_script_path:?}");

    drop(script_file);

    Ok(())
}

/// Adds the service to the default runlevel and starts it.
///
/// # Arguments
///
/// * `name` - Name of the service to enable and start.
///
/// # Errors
///
/// Returns `Err` if the `rc-update` or `rc-service` commands fail.
pub fn start_and_enable_self_as_service(name: &str) -> Result<(), String> {
    Command::new("rc-update")
        .arg("add")
        .arg(name)
        .arg("default")
        .status()
        .map_err_to_string_simple()?;

    Command::new("rc-service")
        .arg(name)
        .arg("start")
        .status()
        .map_err_to_string_simple()?;

    println!("Service {name} started and added to default runlevel.");
    Ok(())
}
