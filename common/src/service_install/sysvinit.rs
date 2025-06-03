use std::{env, fs::{self, File}, io::Write, os::unix::fs::PermissionsExt, path::PathBuf, process::{Command, Stdio}};

use crate::is_superuser;

pub fn install_self_as_service(
    name: &str,
    init_script_content: &str,
) -> Result<(), String> {
    if !is_superuser() {
        return Err("You must run this command as root or with sudo.".to_string());
    }

    let binary_path = PathBuf::from(env::args().next().unwrap());
    let target_bin = PathBuf::from("/usr/sbin/").join(name);
    let init_script_path = format!("/etc/rc.d/rc.{name}");

    // Stop potentially existing service it before overwriting
    let _ = Command::new(&init_script_path)
        .arg("stop")
        .stderr(Stdio::null())
        .status();

    fs::copy(binary_path, &target_bin).map_err(|e| e.to_string())?;
    println!("Installed binary to {target_bin:?}");

    let init_script_content = init_script_content.replace("{binary}", &target_bin.to_string_lossy());

    let mut file = File::create(&init_script_path).map_err(|e| e.to_string())?;
    file.write_all(init_script_content.as_bytes())
        .map_err(|e| e.to_string())?;
    fs::set_permissions(&init_script_path, fs::Permissions::from_mode(0o755))
        .map_err(|e| e.to_string())?;

    drop(file);

    println!("Init script installed at {init_script_path:?}");

    Ok(())
}

pub fn start_and_enable_self_as_service(name: &str) -> Result<(), String> {
    let init_script_path = format!("/etc/rc.d/rc.{name}");

    // Ensure it's added to rc.local
    let rc_local = "/etc/rc.d/rc.local";
    let entry = format!("if [ -x {init_script_path:?} ]; then {init_script_path:?} start; fi\n");
    let rc_local_content = fs::read_to_string(rc_local).unwrap_or_default();
    if !rc_local_content.contains(&entry) {
        let mut file = fs::OpenOptions::new()
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
