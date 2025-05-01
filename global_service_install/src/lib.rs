use std::{env, path::PathBuf, process::Command, os::unix::fs::PermissionsExt};
#[allow(unused_imports)]
use std::path::Path;
use std::{fs::{self, File}, io::Write};

#[cfg(target_os = "linux")]
pub fn install_self_as_service_non_systemd_linux(name: &str, init_script_content: &str) -> Result<(), String> {
    if !is_superuser() {
        return Err("You must run this command as root or with sudo.".to_string());
    }

    let binary_path = PathBuf::from(env::args().next().unwrap());

    let target_bin = PathBuf::from("/usr/sbin/").join(name);

    // Stop potentially existing service it before overwriting
    let init_script = "/etc/rc.d/rc.shuthost_agent";
    if Path::new(init_script).exists() {
        let _ = Command::new(init_script).arg("stop").status();
    }
    fs::copy(binary_path, &target_bin).map_err(|e| e.to_string())?;
    println!("Installed binary to {target_bin:?}");
   
    // fallback for Unraid / Slackware / non-systemd
    let init_script_path = PathBuf::from(format!("/etc/rc.d/rc.{name}"));
    let init_script_content = init_script_content
        .replace("{binary}", &format!("{target_bin}", target_bin = target_bin.to_string_lossy()));


    let mut file = File::create(&init_script_path).map_err(|e| e.to_string())?;
    file.write_all(init_script_content.as_bytes())
        .map_err(|e| e.to_string())?;
    fs::set_permissions(&init_script_path, fs::Permissions::from_mode(0o755))
        .map_err(|e| e.to_string())?;

    // Ensure it's added to rc.local
    let rc_local = "/etc/rc.d/rc.local";
    let entry = format!("if [ -x {init_script_path:?} ]; then {init_script_path:?} start; fi\n");
    let rc_local_content = fs::read_to_string(rc_local).unwrap_or_default();
    if !rc_local_content.contains(&entry) {
        let mut file = File::options()
            .append(true)
            .open(rc_local)
            .map_err(|e| e.to_string())?;
        file.write_all(entry.as_bytes())
            .map_err(|e| e.to_string())?;
    }

    drop(file);

    println!("Init script installed at {init_script_path:?} and added to rc.local.");

    // Start the service now that everything’s in place
    let _ = Command::new(init_script_path)
        .arg("start")
        .status()
        .map_err(|e| format!("Failed to start agent: {e}"))?;

    Ok(())
}

#[cfg(target_os = "linux")]
pub fn install_self_as_service_systemd(name: &str, init_script_content: &str) -> Result<(), String> {
    if !is_superuser() {
        return Err("You must run this command as root or with sudo.".to_string());
    }

    let binary_path = PathBuf::from(env::args().next().unwrap());
    let target_bin = PathBuf::from("/usr/sbin/").join(name);
    let service_name = format!("{name}.service");

    // Stop potentially existing service it before overwriting
    let output = Command::new("systemctl")
        .arg("is-active")
        .arg(&service_name)
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        let _ = Command::new("systemctl")
            .arg("stop")
            .arg(&service_name)
            .status();
    }

    fs::copy(binary_path, &target_bin).map_err(|e| e.to_string())?;
    println!("Installed binary to {target_bin:?}");

    let service_file_path = format!("/etc/systemd/system/{service_name}");
    let service_file_content = init_script_content
        .replace("{binary}", &format!("{target_bin}", target_bin = target_bin.to_string_lossy()));

    let mut service_file = File::create(&service_file_path).map_err(|e| e.to_string())?;
    service_file
        .write_all(service_file_content.as_bytes())
        .map_err(|e| e.to_string())?;
    println!("Created systemd service file at {service_file_path}");

    drop(service_file);

    Command::new("systemctl")
        .arg("daemon-reload")
        .output()
        .map_err(|e| e.to_string())?;
    Command::new("systemctl")
        .arg("enable")
        .arg(&service_name)
        .output()
        .map_err(|e| e.to_string())?;
    Command::new("systemctl")
        .arg("start")
        .arg(&service_name)
        .output()
        .map_err(|e| e.to_string())?;

    println!("Service started and enabled.");

    Ok(())
}

#[cfg(target_os = "macos")]
/// FIXME: DOESNT WORK!!!
pub fn install_self_as_service_macos(name: &str, init_script_content: &str) -> Result<(), String> {
    use std::process::Stdio;

    if !is_superuser() {
        return Err("You must run this command as root or with sudo.".to_string());
    }

    let binary_path = PathBuf::from(env::args().next().unwrap());

    let target_bin = PathBuf::from("/usr/local/bin/").join(name);
    let label = format!("com.github_9smtm6.{name}");
    let plist_path = PathBuf::from(format!("/Library/LaunchDaemons/{label}.plist"));

    fs::copy(binary_path, &target_bin).map_err(|e| e.to_string())?;
    println!("Installed binary to {target_bin:?}");

    // Stop existing job if it's already loaded
    let _ = Command::new("launchctl")
        .arg("unload")
        .arg(&plist_path)
        .stderr(Stdio::null())
        .status();

    let plist_content = init_script_content
        .replace("{name}", &name);

    let mut plist_file = File::create(&plist_path).map_err(|e| e.to_string())?;
    plist_file
        .write_all(plist_content.as_bytes())
        .map_err(|e| e.to_string())?;
    println!("Created launchd plist file at {plist_path:?}");

    drop(plist_file);

    // Set proper permissions
    fs::set_permissions(&plist_path, fs::Permissions::from_mode(0o644))
        .map_err(|e| e.to_string())?;

    // Load and start the daemon
    Command::new("launchctl")
        .arg("load")
        .arg(&plist_path)
        .output()
        .map_err(|e| e.to_string())?;


    println!("Service loaded with launchctl.");

    Ok(())
}

fn is_superuser() -> bool {
    env::var("USER").map(|user| user == "root").unwrap_or(false) || env::var("SUDO_USER").is_ok()
}

#[cfg(target_os = "linux")]
pub fn is_systemd() -> bool {
    Path::new("/run/systemd/system").exists()
}
