use clap::Parser;
use shuthost_common::generate_secret;
#[cfg(target_os = "linux")]
use shuthost_common::is_systemd;
#[allow(unused_imports)]
use std::os::unix::fs::PermissionsExt;
use std::process::Command;
use std::net::UdpSocket;

/// From 5hu7ho57 (leet speak)
pub const DEFAULT_PORT: u16 = 5757;
const CONFIG_ENTRY: &str =
    r#""{name}" = { ip = "{ip}", mac = "{mac}", port = {port}, shared_secret = "{secret}" }"#;
#[cfg(target_os = "linux")]
const SERVICE_FILE_TEMPLATE: &str = include_str!("shuthost_node_agent.service.ini");
#[cfg(target_os = "macos")]
const SERVICE_FILE_TEMPLATE: &str = include_str!("com.github_9smtm6.shuthost_node_agent.plist.xml");
#[cfg(target_os = "linux")]
const SLACKWARE_INIT_TEMPLATE: &str = include_str!("sysvinit.shuthost_node_agent.sh");

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

pub fn install_node_agent(arguments: InstallArgs) -> Result<(), String> {
    let name = env!("CARGO_PKG_NAME");
    let bind_known_vals = |arg: &str| {
        arg.replace("{description}", env!("CARGO_PKG_DESCRIPTION"))
            .replace("{port}", &arguments.port.to_string())
            .replace("{shutdown_command}", &arguments.shutdown_command)
            .replace("{secret}", &arguments.shared_secret)
            .replace("{name}", name)
    };
    #[cfg(target_os = "linux")]
    if is_systemd() {
        shuthost_common::install_self_as_service_systemd(
            &name,
            &bind_known_vals(SERVICE_FILE_TEMPLATE),
        )?;
    } else {
        shuthost_common::install_self_as_service_sysvinit_linux(
            &name,
            &bind_known_vals(SLACKWARE_INIT_TEMPLATE),
        )?;
    }

    #[cfg(target_os = "macos")]
    shuthost_common::install_self_as_service_macos(name, &bind_known_vals(SERVICE_FILE_TEMPLATE))?;

    let interface = &get_default_interface().unwrap();
    println!(
        "Place the following in the coordinator:\n{config_entry}",
        config_entry = CONFIG_ENTRY
            .replace("{name}", &get_hostname().unwrap())
            .replace("{ip}", &get_ip(interface).unwrap())
            .replace("{mac}", &get_mac(interface).unwrap())
            .replace("{port}", &arguments.port.to_string())
            .replace("{secret}", &arguments.shared_secret)
    );

    Ok(())
}

pub fn get_default_shutdown_command() -> String {
    #[cfg(target_os = "linux")]
    return if is_systemd() {
        "systemctl poweroff"
    } else {
        "poweroff"
    }
    .to_string();
    #[cfg(target_os = "macos")]
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
        let output = Command::new("ifconfig").arg(interface).output().ok()?;
        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
            if line.trim_start().starts_with("ether ") {
                return line.split_whitespace().nth(1).map(|s| s.to_string());
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
                return line
                    .trim()
                    .split_whitespace()
                    .nth(1)
                    .and_then(|s| s.split('/').next())
                    .map(|s| s.to_string());
            }
        }
        None
    }

    #[cfg(target_os = "macos")]
    {
        let output = Command::new("ifconfig").arg(interface).output().ok()?;
        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
            if line.trim_start().starts_with("inet ") && !line.contains("127.0.0.1") {
                return line.split_whitespace().nth(1).map(|s| s.to_string());
            }
        }
        None
    }
}

pub fn get_hostname() -> Option<String> {
    let output = Command::new("hostname").output().ok()?;

    let hostname = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if !hostname.is_empty() {
        Some(hostname)
    } else {
        None
    }
}

pub fn test_wol_reachability(port: u16) -> Result<(), String> {
    let socket = UdpSocket::bind(format!("0.0.0.0:{}", port))
        .map_err(|e| format!("Failed to bind test socket: {}", e))?;
    
    socket.set_broadcast(true)
        .map_err(|e| format!("Failed to set broadcast: {}", e))?;

    println!("Listening for WOL test packets on port {}...", port);
    
    let mut buf = [0u8; 32];
    for _ in 0..2 {  // Wait for both direct and broadcast tests
        if let Ok((_, addr)) = socket.recv_from(&mut buf) {
            // Echo back to confirm receipt
            socket.send_to(b"SHUTHOST_AGENT RECEIVED", addr)
                .map_err(|e| format!("Failed to send confirmation: {}", e))?;
        }
    }

    Ok(())
}
