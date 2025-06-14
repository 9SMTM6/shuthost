use std::{
    env,
    fs::{self, File},
    io::Write,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use crate::is_superuser;

pub fn install_self_as_service(name: &str, init_script_content: &str) -> Result<(), String> {
    if !is_superuser() {
        return Err("You must run this command as root or with sudo.".to_string());
    }

    let binary_path = env::current_exe().unwrap();
    let target_bin = Path::new("/usr/sbin/").join(name);
    let init_script_path = PathBuf::from(format!("/etc/init.d/{name}"));

    // Stop and remove any existing service
    let _ = Command::new("rc-service")
        .arg(name)
        .arg("stop")
        .stderr(Stdio::null())
        .status();

    fs::copy(&binary_path, &target_bin).map_err(|e| e.to_string())?;
    println!("Installed binary to {:?}", target_bin);
    // Set binary permissions to 0755 (root can write, others can read/execute)
    fs::set_permissions(&target_bin, fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;

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
    perms.set_mode(0o750);
    fs::set_permissions(&init_script_path, perms).map_err(|e| e.to_string())?;
    println!("Created OpenRC init script at {:?}", init_script_path);

    drop(script_file);

    Ok(())
}

pub fn start_and_enable_self_as_service(name: &str) -> Result<(), String> {
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
