//! Installation and runtime utilities for the host_agent binary.
//!
//! Handles subcommand parsing, agent installation across init systems, network interface discovery, and Wake-on-LAN testing.

use clap::Parser;
use shuthost_common::generate_secret;
#[cfg(target_os = "linux")]
use shuthost_common::{is_openrc, is_systemd};
use std::net::UdpSocket;
use std::process::Command;

/// Default UDP port on which the host_agent listens for commands.
pub const DEFAULT_PORT: u16 = 5757;

/// Template string for adding an agent entry in coordinator configuration.
const CONFIG_ENTRY: &str =
    r#""{name}" = { ip = "{ip}", mac = "{mac}", port = {port}, shared_secret = "{secret}" }"#;
#[cfg(target_os = "linux")]
const SERVICE_FILE_TEMPLATE: &str = include_str!("shuthost_host_agent.service.ini");
#[cfg(target_os = "macos")]
const SERVICE_FILE_TEMPLATE: &str = include_str!("com.github_9smtm6.shuthost_host_agent.plist.xml");
#[cfg(target_os = "linux")]
const OPENRC_FILE_TEMPLATE: &str = include_str!("openrc.shuthost_host_agent.sh");

/// Arguments for the `install` subcommand of host_agent.
#[derive(Debug, Parser)]
pub struct InstallArgs {
    #[arg(long = "port", default_value_t = DEFAULT_PORT)]
    pub port: u16,

    #[arg(long = "shutdown-command", default_value_t = get_default_shutdown_command())]
    pub shutdown_command: String,

    #[arg(long = "shared-secret", default_value_t = generate_secret())]
    pub shared_secret: String,

    #[arg(long = "init-system", default_value_t = get_inferred_init_system())]
    pub init_system: InitSystem,
}

/// Supported init systems for installing the host_agent.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum InitSystem {
    /// Systemd init system (Linux).
    #[cfg(target_os = "linux")]
    Systemd,
    /// OpenRC init system (Linux).
    #[cfg(target_os = "linux")]
    OpenRC,
    /// No init system; generates a self-extracting script you'll have to start yourself.
    Serviceless,
    /// Launchd init system (macOS).
    #[cfg(target_os = "macos")]
    Launchd,
}

impl std::fmt::Display for InitSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match *self {
            #[cfg(target_os = "linux")]
            InitSystem::Systemd => "systemd",
            #[cfg(target_os = "linux")]
            InitSystem::OpenRC => "open-rc",
            InitSystem::Serviceless => "serviceless",
            #[cfg(target_os = "macos")]
            InitSystem::Launchd => "launchd",
        };
        write!(f, "{}", s)
    }
}

/// Performs host_agent installation based on provided arguments.
///
/// Selects and invokes the appropriate init system installer or generates a script.
pub fn install_host_agent(arguments: &InstallArgs) -> Result<(), String> {
    let name = env!("CARGO_PKG_NAME");
    let bind_known_vals = |arg: &str| {
        arg.replace("{description}", env!("CARGO_PKG_DESCRIPTION"))
            .replace("{port}", &arguments.port.to_string())
            .replace("{shutdown_command}", &arguments.shutdown_command)
            .replace("{secret}", &arguments.shared_secret)
            .replace("{name}", name)
    };

    match arguments.init_system {
        #[cfg(target_os = "linux")]
        InitSystem::Systemd => {
            shuthost_common::systemd::install_self_as_service(
                &name,
                &bind_known_vals(SERVICE_FILE_TEMPLATE),
            )?;
            shuthost_common::systemd::start_and_enable_self_as_service(&name)?;
        }
        #[cfg(target_os = "linux")]
        InitSystem::OpenRC => {
            shuthost_common::openrc::install_self_as_service(
                &name,
                &bind_known_vals(OPENRC_FILE_TEMPLATE),
            )?;
            shuthost_common::openrc::start_and_enable_self_as_service(&name)?;
        }
        InitSystem::Serviceless => {
            let target_script_path = format!("./{name}_self_extracting");
            shuthost_common::serviceless::generate_self_extracting_script(
                &[
                    ("SHUTHOST_SHARED_SECRET", &arguments.shared_secret),
                    ("PORT", &arguments.port.to_string()),
                    ("SHUTDOWN_COMMAND", &arguments.shutdown_command),
                ],
                "service --port=\"$PORT\" --shutdown-command=\"$SHUTDOWN_COMMAND\"",
                &target_script_path,
            )?;
            // Start the self-extracting script in the background
            if let Err(e) = std::process::Command::new(&target_script_path).output() {
                eprintln!("Failed to start self-extracting script: {}", e);
            } else {
                println!("Started self-extracting agent script in background.");
            }
        }
        #[cfg(target_os = "macos")]
        InitSystem::Launchd => {
            shuthost_common::macos::install_self_as_service(
                name,
                &bind_known_vals(SERVICE_FILE_TEMPLATE),
            )?;
            shuthost_common::macos::start_and_enable_self_as_service(name)?;
        }
    }

    let interface = &get_default_interface();
    if interface.is_none() {
        eprintln!(
            "Failed to determine the default network interface. Continuing on assuming docker or similar environment."
        );
    }
    println!(
        "Place the following in the coordinator:\n{config_entry}",
        config_entry = CONFIG_ENTRY
            .replace("{name}", &get_hostname().unwrap())
            .replace(
                "{ip}",
                &interface
                    .as_ref()
                    .and_then(|it| get_ip(it))
                    .unwrap_or("unrecognized".to_string())
            )
            .replace(
                "{mac}",
                &interface
                    .as_ref()
                    .and_then(|it| get_mac(it))
                    .unwrap_or("unrecognized".to_string())
            )
            .replace("{port}", &arguments.port.to_string())
            .replace("{secret}", &arguments.shared_secret)
    );

    Ok(())
}

/// Auto-detects the host system's init system.
#[cfg_attr(
    target_os = "macos",
    expect(
        clippy::missing_const_for_fn,
        reason = "can't be const because of linux"
    )
)]
fn get_inferred_init_system() -> InitSystem {
    #[cfg(target_os = "linux")]
    {
        if is_systemd() {
            InitSystem::Systemd
        } else if is_openrc() {
            InitSystem::OpenRC
        } else {
            InitSystem::Serviceless
        }
    }
    #[cfg(target_os = "macos")]
    {
        InitSystem::Launchd
    }
}

/// Returns the default shutdown command for this OS and init system.
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

/// Attempts to determine the default network interface by parsing system routing information.
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

/// Retrieves the MAC address for the named network interface.
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

/// Retrieves the IP address for the named network interface.
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

/// Retrieves the system hostname.
pub fn get_hostname() -> Option<String> {
    let output = Command::new("hostname").output().ok()?;

    let hostname = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if !hostname.is_empty() {
        Some(hostname)
    } else {
        None
    }
}

/// Tests Wake-on-LAN packet reachability by listening and echoing back packets.
pub fn test_wol_reachability(port: u16) -> Result<(), String> {
    let socket = UdpSocket::bind(format!("0.0.0.0:{}", port))
        .map_err(|e| format!("Failed to bind test socket: {}", e))?;

    socket
        .set_broadcast(true)
        .map_err(|e| format!("Failed to set broadcast: {}", e))?;

    println!("Listening for WOL test packets on port {}...", port);

    let mut buf = [0u8; 32];
    for _ in 0..2 {
        // Wait for both direct and broadcast tests
        if let Ok((_, addr)) = socket.recv_from(&mut buf) {
            // Echo back to confirm receipt
            socket
                .send_to(b"SHUTHOST_AGENT RECEIVED", addr)
                .map_err(|e| format!("Failed to send confirmation: {}", e))?;
        }
    }

    Ok(())
}
