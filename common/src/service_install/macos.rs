use std::path::PathBuf;
use std::{
    env,
    fs::{self, File},
    io::Write,
    os::unix::fs::PermissionsExt,
    process::{Command, Stdio},
};

use crate::is_superuser;

pub fn install_self_as_service(name: &str, init_script_content: &str) -> Result<(), String> {
    if !is_superuser() {
        return Err("You must run this command as root or with sudo.".to_string());
    }

    let binary_path = env::current_exe().unwrap();

    let target_bin = PathBuf::from("/usr/local/bin/").join(name);
    let label = format!("com.github_9smtm6.{name}");
    let plist_path = PathBuf::from(format!("/Library/LaunchDaemons/{label}.plist"));

    fs::copy(binary_path, &target_bin).map_err(|e| e.to_string())?;
    println!("Installed binary to {target_bin:?}");
    // Set binary permissions to 0755 (root can write, others can read/execute)
    fs::set_permissions(&target_bin, fs::Permissions::from_mode(0o755))
        .map_err(|e| e.to_string())?;

    // Stop existing job if it's already loaded (modern launchctl)
    if let Ok(_) = Command::new("launchctl")
        .arg("bootout")
        .arg("system")
        .arg(&plist_path)
        .stderr(Stdio::null())
        .status()
    {
        println!("Stopped existing service");
    };

    let plist_content = init_script_content.replace("{name}", name);

    let mut plist_file = File::create(&plist_path).map_err(|e| e.to_string())?;
    plist_file
        .write_all(plist_content.as_bytes())
        .map_err(|e| e.to_string())?;
    println!("Created launchd plist file at {plist_path:?}");

    drop(plist_file);

    // Set proper permissions
    fs::set_permissions(&plist_path, fs::Permissions::from_mode(0o640))
        .map_err(|e| e.to_string())?;

    Ok(())
}

pub fn start_and_enable_self_as_service(name: &str) -> Result<(), String> {
    let label = format!("com.github_9smtm6.{name}");
    let plist_path = PathBuf::from(format!("/Library/LaunchDaemons/{label}.plist"));

    // Load and start the daemon (modern launchctl)
    Command::new("launchctl")
        .arg("bootstrap")
        .arg("system")
        .arg(&plist_path)
        .output()
        .map_err(|e| e.to_string())?;

    println!("Service bootstrapped with launchctl.");

    // Optionally print the service status
    let status = Command::new("launchctl")
        .arg("print")
        .arg(format!("system/{}", label))
        .output()
        .map_err(|e| e.to_string())?;

    if status.status.success() {
        println!(
            "Service status:\n{}",
            String::from_utf8_lossy(&status.stdout)
        );
    } else {
        println!(
            "Failed to print service status:\n{}",
            String::from_utf8_lossy(&status.stderr)
        );
    }

    Ok(())
}
