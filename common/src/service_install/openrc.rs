//! `OpenRC` service installer for Unix-like systems.
//!
//! Provides functions to install the current binary as an `OpenRC` init script and start it.

use std::{
    env,
    fs::{self, File},
    io::Write as _,
    os::unix::fs::PermissionsExt as _,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use crate::{ResultMapErrExt as _, is_superuser};

/// Returns the `OpenRC` service file path for the given service name.
#[must_use]
pub fn get_service_path(name: &str) -> String {
    format!("/etc/init.d/{name}")
}

/// Installs the current binary as an `OpenRC` service init script.
///
/// # Arguments
///
/// * `name` - Name to assign to the service and executable.
/// * `init_script_content` - Template for the `OpenRC` init script (with `{ binary }` placeholder).
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
    println!("DEBUG: running rc-service {} stop", name);
    let stop_output = Command::new("rc-service")
        .arg(name)
        .arg("stop")
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .output();

    match stop_output {
        Ok(output) => {
            println!("DEBUG: rc-service stop status={}", output.status);
            println!("DEBUG: rc-service stop stdout={:?}", String::from_utf8_lossy(&output.stdout));
            println!("DEBUG: rc-service stop stderr={:?}", String::from_utf8_lossy(&output.stderr));
            if output.status.success() {
                println!("Stopped existing service {name}.");
            } else {
                println!("Service {name} stop returned status: {}", output.status);
            }
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
    println!("DEBUG: running rc-update add {} default", name);
    let add_output = Command::new("rc-update")
        .arg("add")
        .arg(name)
        .arg("default")
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .output()
        .map_err_to_string_simple()?;
    println!("DEBUG: rc-update add status={}", add_output.status);
    println!("DEBUG: rc-update add stdout={:?}", String::from_utf8_lossy(&add_output.stdout));
    println!("DEBUG: rc-update add stderr={:?}", String::from_utf8_lossy(&add_output.stderr));

    println!("DEBUG: running rc-service {} start", name);
    let start_output = Command::new("rc-service")
        .arg(name)
        .arg("start")
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .output()
        .map_err_to_string_simple()?;
    println!("DEBUG: rc-service start status={}", start_output.status);
    println!("DEBUG: rc-service start stdout={:?}", String::from_utf8_lossy(&start_output.stdout));
    println!("DEBUG: rc-service start stderr={:?}", String::from_utf8_lossy(&start_output.stderr));

    println!("Service {name} started and added to default runlevel.");
    Ok(())
}
