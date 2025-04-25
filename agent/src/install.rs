use std::env;
use std::fs::{self, File};
use std::io::{Write, Read};
use std::process::Command;
use std::path::Path;
use rand::Rng;

const DEFAULT_PORT: u16 = 9090;
const DEFAULT_SHUTDOWN_COMMAND: &str = "systemctl poweroff";
const SERVICE_FILE_TEMPLATE: &str = r#"
[Unit]
Description=Agent for Remote Management

[Service]
ExecStart=/usr/sbin/agent --port={port} --shutdown-command={shutdown_command} --shared-secret={secret}
Restart=always
User=root
Group=root

[Install]
WantedBy=multi-user.target
"#;

pub fn install_agent(install_path: &Path, port: Option<u16>, shutdown_command: Option<&str>, secret: Option<String>) -> Result<(), String> {
    // Check for superuser rights
    if !is_superuser() {
        return Err("You must run this command as root or with sudo.".to_string());
    }
    
    // Generate secret if not provided
    let secret = secret.unwrap_or_else(|| generate_secret());
    
    // Prepare paths
    let target_bin = "/usr/sbin/agent";
    let service_file_path = "/etc/systemd/system/agent.service";
    
    // Move the agent binary to /usr/sbin/
    fs::copy(install_path, target_bin).map_err(|e| e.to_string())?;
    println!("[agent] Installed binary to /usr/sbin/agent");

    // Create systemd service file
    let service_file_content = SERVICE_FILE_TEMPLATE
        .replace("{port}", &port.unwrap_or(DEFAULT_PORT).to_string())
        .replace("{shutdown_command}", shutdown_command.unwrap_or(DEFAULT_SHUTDOWN_COMMAND))
        .replace("{secret}", &secret);
    
    let mut service_file = File::create(service_file_path).map_err(|e| e.to_string())?;
    service_file.write_all(service_file_content.as_bytes()).map_err(|e| e.to_string())?;
    println!("[agent] Created systemd service file at /etc/systemd/system/agent.service");

    // Enable and start the service
    Command::new("systemctl")
        .arg("daemon-reload")
        .output()
        .map_err(|e| e.to_string())?;
    
    Command::new("systemctl")
        .arg("enable")
        .arg("agent.service")
        .output()
        .map_err(|e| e.to_string())?;
    
    Command::new("systemctl")
        .arg("start")
        .arg("agent.service")
        .output()
        .map_err(|e| e.to_string())?;

    println!("[agent] Service started and enabled.");
    
    Ok(())
}

fn is_superuser() -> bool {
    env::var("USER").map(|user| user == "root").unwrap_or(false) || env::var("SUDO_USER").is_ok()
}

fn generate_secret() -> String {
    // Simple random secret generation: 32 characters
    let mut rng = rand::thread_rng();
    let chars: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789".chars().collect();
    (0..32)
        .map(|_| chars[rng.gen_range(0..chars.len())])
        .collect()
}
