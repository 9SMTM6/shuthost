use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::process::Command;
use std::path::Path;
use rand::Rng;
use clap::Parser;
#[allow(unused_imports)]
use std::os::unix::fs::PermissionsExt;

const DEFAULT_PORT: u16 = 9090;
const CONFIG_ENTRY: &str = r#""{name}" = { ip = "{ip}", mac = "{mac}", port = {port}, shared_secret = "{secret}" }"#;
#[cfg(target_os = "linux")]
const SERVICE_FILE_TEMPLATE: &str = include_str!("shutdown_agent.service.ini");
#[cfg(target_os = "macos")]
const SERVICE_FILE_TEMPLATE: &str = include_str!("com.github.9smtm6.shutdown_agent.plist.xml");
#[cfg(target_os = "linux")]
const SLACKWARE_INIT_TEMPLATE: &str = include_str!("rc.shuthost_agent.sh");

/// Struct for the install subcommand, with defaults added
#[derive(Debug, Parser)]
pub struct InstallArgs {
    #[arg(long = "port", default_value_t = DEFAULT_PORT)]
    pub port: u16,

    #[arg(long = "shutdown-command", default_value_t = get_default_shutdown_command())]
    pub shutdown_command: String,

    #[arg(long = "shared-secret", default_value_t = generate_secret())]
    pub shared_secret: String,
}

pub fn install_agent(install_path: &Path, arguments: InstallArgs) -> Result<(), String> {
    if !is_superuser() {
        return Err("You must run this command as root or with sudo.".to_string());
    }

    #[cfg(target_os = "linux")]
    {
        let target_bin = "/usr/sbin/shuthost_agent";

        fs::copy(install_path, target_bin).map_err(|e| e.to_string())?;
        println!("Installed binary to {target_bin}");

        if is_systemd() {
            let service_name = "shuthost_agent.service";
            let service_file_path = format!("/etc/systemd/system/{service_name}");
            let service_file_content = SERVICE_FILE_TEMPLATE
                .replace("{description}", env!("CARGO_PKG_DESCRIPTION"))
                .replace("{port}", &arguments.port.to_string())
                .replace("{shutdown_command}", &arguments.shutdown_command)
                .replace("{secret}", &arguments.shared_secret)
                .replace("{binary}", target_bin);

            let mut service_file = File::create(&service_file_path).map_err(|e| e.to_string())?;
            service_file.write_all(service_file_content.as_bytes()).map_err(|e| e.to_string())?;
            println!("Created systemd service file at {service_file_path}");

            Command::new("systemctl").arg("daemon-reload").output().map_err(|e| e.to_string())?;
            Command::new("systemctl").arg("enable").arg(service_name).output().map_err(|e| e.to_string())?;
            Command::new("systemctl").arg("start").arg(service_name).output().map_err(|e| e.to_string())?;

            println!("Service started and enabled.");
        } else {
            // fallback for Unraid / Slackware / non-systemd
            let init_script_path = "/etc/rc.d/rc.shuthost_agent";
            let init_script_content = SLACKWARE_INIT_TEMPLATE
                .replace("{description}", env!("CARGO_PKG_DESCRIPTION"))
                .replace("{port}", &arguments.port.to_string())
                .replace("{shutdown_command}", &arguments.shutdown_command)
                .replace("{secret}", &arguments.shared_secret)
                .replace("{binary}", target_bin);
        
            let mut file = File::create(init_script_path).map_err(|e| e.to_string())?;
            file.write_all(init_script_content.as_bytes()).map_err(|e| e.to_string())?;
            fs::set_permissions(init_script_path, fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
        
            // Ensure it's added to rc.local
            let rc_local = "/etc/rc.d/rc.local";
            let entry = "if [ -x /etc/rc.d/rc.shuthost_agent ]; then /etc/rc.d/rc.shuthost_agent start; fi\n";
            let rc_local_content = fs::read_to_string(rc_local).unwrap_or_default();
            if !rc_local_content.contains("rc.shuthost_agent start") {
                let mut file = File::options().append(true).open(rc_local).map_err(|e| e.to_string())?;
                file.write_all(entry.as_bytes()).map_err(|e| e.to_string())?;
            }
        
            println!("Init script installed at {init_script_path} and added to rc.local.");

            // Start the service immediately after install
            Command::new("/etc/rc.d/rc.shuthost_agent")
                .arg("start")
                .status()
                .map_err(|e| format!("Failed to start agent: {e}"))?;
        }
        
    }

    #[cfg(target_os = "macos")]
    {
        let target_bin = "/usr/local/bin/shuthost_agent";
        let plist_path = "/Library/LaunchDaemons/com.github.9smtm6.shutdown_agent.plist";

        fs::copy(install_path, target_bin).map_err(|e| e.to_string())?;
        println!("Installed binary to {target_bin}");

        let plist_content = SERVICE_FILE_TEMPLATE
            .replace("{description}", env!("CARGO_PKG_DESCRIPTION"))
            .replace("{port}", &arguments.port.to_string())
            .replace("{shutdown_command}", &arguments.shutdown_command)
            .replace("{secret}", &arguments.shared_secret)
            .replace("{binary}", target_bin);

        let mut plist_file = File::create(plist_path).map_err(|e| e.to_string())?;
        plist_file.write_all(plist_content.as_bytes()).map_err(|e| e.to_string())?;
        println!("Created launchd plist file at {plist_path}");

        Command::new("launchctl").arg("load").arg(plist_path).output().map_err(|e| e.to_string())?;
        println!("Service loaded with launchctl.");
    }

    let interface = &get_default_interface().unwrap();
    print!(
        "Place the following in the controller:\n{config_entry}",
        config_entry = CONFIG_ENTRY
            .replace("{name}", &get_hostname().unwrap())
            .replace("{ip}", &get_ip(interface).unwrap())
            .replace("{mac}", &get_mac(interface).unwrap())
            .replace("{port}", &arguments.port.to_string())
            .replace("{secret}", &arguments.shared_secret)
    );

    Ok(())
}

fn is_superuser() -> bool {
    env::var("USER").map(|user| user == "root").unwrap_or(false) || env::var("SUDO_USER").is_ok()
}

pub fn generate_secret() -> String {
    // Simple random secret generation: 32 characters
    let mut rng = rand::rng();
    let chars: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789".chars().collect();
    (0..32)
        .map(|_| chars[rng.random_range(0..chars.len())])
        .collect::<String>()
        .into()
}

#[cfg(target_os="linux")]
fn is_systemd() -> bool {
    Path::new("/run/systemd/system").exists()
}

fn get_default_shutdown_command() -> String {
    #[cfg(target_os="linux")]
    return if is_systemd() {
        "systemctl poweroff"
    } else {
        "poweroff"
    }.to_string();
    #[cfg(target_os="macos")]
    return "shutdown -h now".to_string();
}

fn get_default_interface() -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        let output = Command::new("ip")
            .args(["route", "show", "default"])
            .output()
            .ok()?;

        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
            if line.starts_with("default") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 5 {
                    return Some(parts[4].to_string());
                }
            }
        }
        None
    }

    #[cfg(target_os = "macos")]
    {
        let output = Command::new("route")
            .args(["get", "default"])
            .output()
            .ok()?;

        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
            if line.trim_start().starts_with("interface:") {
                return line.split(':').nth(1).map(|s| s.trim().to_string());
            }
        }
        None
    }
}

pub fn get_mac(interface: &str) -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        let output = Command::new("ip")
            .args(["link", "show", interface])
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
            if line.contains("ether") {
                return line.trim().split_whitespace().nth(1).map(|s| s.to_string());
            }
        }
        None
    }

    #[cfg(target_os = "macos")]
    {
        let output = Command::new("ifconfig")
            .arg(interface)
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
            if line.trim_start().starts_with("ether ") {
                return line.trim().split_whitespace().nth(1).map(|s| s.to_string());
            }
        }
        None
    }
}

pub fn get_ip(interface: &str) -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        let output = Command::new("ip")
            .args(["addr", "show", interface])
            .output()
            .ok()?;

        let text = String::from_utf8_lossy(&output.stdout);

        for line in text.lines() {
            // Looking for the line that contains 'inet', which is typically the IP address line
            if line.contains("inet ") {
                return line.trim().split_whitespace().nth(1)
                    .and_then(|s| s.split('/').next())
                    .map(|s| s.to_string());
            }
        }
        None
    }

    #[cfg(target_os = "macos")]
    {
        let output = Command::new("ifconfig")
            .arg(interface)
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
            if line.trim_start().starts_with("inet ") && !line.contains("127.0.0.1") {
                return line.trim().split_whitespace().nth(1).map(|s| s.to_string());
            }
        }
        None
    }
}

pub fn get_hostname() -> Option<String> {
    let output = Command::new("hostname")
        .output()
        .ok()?;
    
    let hostname = String::from_utf8_lossy(&output.stdout).trim().to_string();
    
    if !hostname.is_empty() {
        Some(hostname)
    } else {
        None
    }
}
