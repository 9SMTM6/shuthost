#[allow(unused_imports)]
use std::path::Path;
use std::{env, os::unix::fs::PermissionsExt, path::PathBuf, process::Command};
use std::{
    fs::{self, File},
    io::Write,
    process::Stdio,
};

#[cfg(target_os = "linux")]
pub fn install_self_as_service_sysvinit_linux(
    name: &str,
    init_script_content: &str,
) -> Result<(), String> {
    if !is_superuser() {
        return Err("You must run this command as root or with sudo.".to_string());
    }

    let binary_path = PathBuf::from(env::args().next().unwrap());

    let target_bin = PathBuf::from("/usr/sbin/").join(name);

    // Stop potentially existing service it before overwriting
    let init_script = format!("/etc/rc.d/rc.{name}");
    let _ = Command::new(init_script)
        .arg("stop")
        .stderr(Stdio::null())
        .status();

    fs::copy(binary_path, &target_bin).map_err(|e| e.to_string())?;
    println!("Installed binary to {target_bin:?}");

    // fallback for Unraid / Slackware / non-systemd
    let init_script_path = PathBuf::from(format!("/etc/rc.d/rc.{name}"));
    let init_script_content =
        init_script_content.replace("{binary}", &target_bin.to_string_lossy());

    let mut file = File::create(&init_script_path).map_err(|e| e.to_string())?;
    file.write_all(init_script_content.as_bytes())
        .map_err(|e| e.to_string())?;
    fs::set_permissions(&init_script_path, fs::Permissions::from_mode(0o755))
        .map_err(|e| e.to_string())?;

    drop(file);

    println!("Init script installed at {init_script_path:?}");

    Ok(())
}

#[cfg(target_os = "linux")]
pub fn install_self_as_service_systemd(
    name: &str,
    init_script_content: &str,
) -> Result<(), String> {
    if !is_superuser() {
        return Err("You must run this command as root or with sudo.".to_string());
    }

    let binary_path = PathBuf::from(env::args().next().unwrap());
    let target_bin = PathBuf::from("/usr/sbin/").join(name);
    let service_name = format!("{name}.service");

    // Stop potentially existing service it before overwriting
    let _ = Command::new("systemctl")
        .arg("stop")
        .arg(&service_name)
        .stderr(Stdio::null())
        .status();

    fs::copy(binary_path, &target_bin).map_err(|e| e.to_string())?;
    println!("Installed binary to {target_bin:?}");

    let service_file_path = format!("/etc/systemd/system/{service_name}");
    let service_file_content =
        init_script_content.replace("{binary}", &target_bin.to_string_lossy());

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

    Ok(())
}

#[cfg(target_os = "linux")]
pub fn install_self_as_service_openrc_linux(
    name: &str,
    init_script_content: &str,
) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    if !is_superuser() {
        return Err("You must run this command as root or with sudo.".to_string());
    }

    let binary_path = PathBuf::from(env::args().next().unwrap());
    let target_bin = PathBuf::from("/usr/sbin/").join(name);
    let init_script_path = PathBuf::from(format!("/etc/init.d/{name}"));

    // Stop and remove any existing service
    let _ = Command::new("rc-service")
        .arg(name)
        .arg("stop")
        .stderr(Stdio::null())
        .status();

    fs::copy(&binary_path, &target_bin).map_err(|e| e.to_string())?;
    println!("Installed binary to {:?}", target_bin);

    let init_script_content =
        init_script_content.replace("{binary}", &target_bin.to_string_lossy());

    let mut script_file = File::create(&init_script_path).map_err(|e| e.to_string())?;
    script_file
        .write_all(init_script_content.as_bytes())
        .map_err(|e| e.to_string())?;

    let mut perms = script_file
        .metadata()
        .map_err(|e| e.to_string())?
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&init_script_path, perms).map_err(|e| e.to_string())?;
    println!("Created OpenRC init script at {:?}", init_script_path);

    drop(script_file);

    Ok(())
}

#[cfg(target_os = "macos")]
/// FIXME: DOESNT WORK!!!
pub fn install_self_as_service_macos(name: &str, init_script_content: &str) -> Result<(), String> {
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

    let plist_content = init_script_content.replace("{name}", name);

    let mut plist_file = File::create(&plist_path).map_err(|e| e.to_string())?;
    plist_file
        .write_all(plist_content.as_bytes())
        .map_err(|e| e.to_string())?;
    println!("Created launchd plist file at {plist_path:?}");

    drop(plist_file);

    // Set proper permissions
    fs::set_permissions(&plist_path, fs::Permissions::from_mode(0o644))
        .map_err(|e| e.to_string())?;

    Ok(())
}

pub fn start_and_enable_self_as_service_macos(name: &str) -> Result<(), String> {
    let label = format!("com.github_9smtm6.{name}");
    let plist_path = PathBuf::from(format!("/Library/LaunchDaemons/{label}.plist"));
    // Load and start the daemon
    Command::new("launchctl")
        .arg("load")
        .arg(&plist_path)
        .output()
        .map_err(|e| e.to_string())?;

    println!("Service loaded with launchctl.");
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn start_and_enable_self_as_service_systemd(name: &str) -> Result<(), String> {
    let service_name = format!("{name}.service");

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

    println!("Service {service_name} started and enabled.");
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn start_and_enable_self_as_service_openrc_linux(name: &str) -> Result<(), String> {
    Command::new("rc-update")
        .arg("add")
        .arg(name)
        .arg("default")
        .status()
        .map_err(|e| e.to_string())?;

    Command::new("rc-service")
        .arg(name)
        .arg("start")
        .status()
        .map_err(|e| e.to_string())?;

    println!("Service {name} started and added to default runlevel.");
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn start_and_enable_self_as_service_sysvinit_linux(name: &str) -> Result<(), String> {
    let init_script_path = format!("/etc/rc.d/rc.{name}");

    // Ensure it's added to rc.local
    let rc_local = "/etc/rc.d/rc.local";
    let entry = format!("if [ -x {init_script_path:?} ]; then {init_script_path:?} start; fi\n");
    let rc_local_content = std::fs::read_to_string(rc_local).unwrap_or_default();
    if !rc_local_content.contains(&entry) {
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(rc_local)
            .map_err(|e| e.to_string())?;
        file.write_all(entry.as_bytes())
            .map_err(|e| e.to_string())?;
    }

    // Start the service
    Command::new(&init_script_path)
        .arg("start")
        .status()
        .map_err(|e| format!("Failed to start service: {e}"))?;

    println!("Service {name} started and enabled via rc.local.");
    Ok(())
}

fn is_superuser() -> bool {
    unsafe { libc::geteuid() == 0 }
}

#[cfg(target_os = "linux")]
pub fn is_systemd() -> bool {
    Path::new("/run/systemd/system").exists()
}

#[cfg(target_os = "linux")]
pub fn is_openrc() -> bool {
    Path::new("/run/openrc").exists() || Path::new("/etc/init.d").exists()
}

#[cfg(target_os = "linux")]
pub fn is_sysvinit() -> bool {
    Path::new("/etc/rc.d").exists() && !is_systemd() && !is_openrc()
}
