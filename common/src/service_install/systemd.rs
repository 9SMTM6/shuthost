use std::{
    env,
    fs::{self, File},
    io::Write,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::{Command, Stdio},
};

use crate::is_superuser;

pub fn install_self_as_service(name: &str, init_script_content: &str) -> Result<(), String> {
    if !is_superuser() {
        return Err("You must run this command as root or with sudo.".to_string());
    }

    let binary_path = env::current_exe().unwrap();
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
    // Set binary permissions to 0755 (root can write, others can read/execute)
    fs::set_permissions(&target_bin, fs::Permissions::from_mode(0o755))
        .map_err(|e| e.to_string())?;

    let service_file_path = format!("/etc/systemd/system/{service_name}");
    let service_file_content =
        init_script_content.replace("{binary}", &target_bin.to_string_lossy());

    let mut service_file = File::create(&service_file_path)
        .map_err(|e| e.to_string())
        .unwrap();
    service_file
        .write_all(service_file_content.as_bytes())
        .map_err(|e| e.to_string())
        .unwrap();
    // Set service file permissions to 0640 (root:root)
    fs::set_permissions(&service_file_path, fs::Permissions::from_mode(0o640))
        .map_err(|e| e.to_string())
        .unwrap();
    println!("Created systemd service file at {service_file_path}");

    drop(service_file);

    Command::new("systemctl")
        .arg("daemon-reload")
        .output()
        .unwrap();

    Ok(())
}

pub fn start_and_enable_self_as_service(name: &str) -> Result<(), String> {
    let service_name = format!("{name}.service");

    Command::new("systemctl")
        .arg("daemon-reload")
        .output()
        .unwrap();

    Command::new("systemctl")
        .arg("enable")
        .arg(&service_name)
        .output()
        .unwrap();

    Command::new("systemctl")
        .arg("start")
        .arg(&service_name)
        .output()
        .unwrap();

    println!("Service {service_name} started and enabled.");
    Ok(())
}
