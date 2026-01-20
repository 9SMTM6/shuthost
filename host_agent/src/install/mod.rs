//! Installation and runtime utilities for the host_agent binary.
//!
//! Handles subcommand parsing, agent installation across init systems, network interface discovery, and Wake-on-LAN testing.

pub mod self_extracting;

use clap::Parser;
use shuthost_common::generate_secret;
#[cfg(target_os = "linux")]
use shuthost_common::{is_openrc, is_systemd};
use std::net::UdpSocket;
use std::process::Command;

use crate::{DEFAULT_PORT, registration, server::get_default_shutdown_command};

/// The binary name, derived from the Cargo package name.
pub(super) const BINARY_NAME: &str = env!("CARGO_PKG_NAME");
#[cfg(any(target_os = "linux", test))]
pub(crate) const SYSTEMD_SERVICE_FILE_TEMPLATE: &str =
    include_str!("shuthost_host_agent.service.ini");
#[cfg(any(target_os = "macos", test))]
pub(crate) const LAUNCHD_SERVICE_FILE_TEMPLATE: &str =
    include_str!("com.github_9smtm6.shuthost_host_agent.plist.xml");
#[cfg(any(target_os = "linux", test))]
pub(crate) const OPENRC_SERVICE_FILE_TEMPLATE: &str = include_str!("openrc.shuthost_host_agent.sh");

/// Binds template placeholders with actual values.
pub(crate) fn bind_template_replacements(
    template: &str,
    description: &str,
    port: &str,
    shutdown_command: &str,
    secret: &str,
) -> String {
    template
        .replace("{ description }", description)
        .replace("{ port }", port)
        .replace("{ shutdown_command }", shutdown_command)
        .replace("{ secret }", secret)
        .replace("{ name }", BINARY_NAME)
}

/// Arguments for the `install` subcommand of host_agent.
#[derive(Debug, Parser)]
pub struct Args {
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
#[derive(Debug, Clone, Copy, clap::ValueEnum, PartialEq)]
pub enum InitSystem {
    /// Systemd init system (Linux).
    #[cfg(target_os = "linux")]
    Systemd,
    /// OpenRC init system (Linux).
    #[cfg(target_os = "linux")]
    OpenRC,
    /// Generates a self-extracting shell script that embeds the compiled binary. The purpose is to keep the configuration readable (and editable) while being a single file that can be managed as one unit. You'll have to start the script yourself.
    #[cfg(unix)]
    SelfExtractingShell,
    /// Generates a self-extracting PowerShell script that embeds the compiled binary. The purpose is to keep the configuration readable (and editable) while being a single file that can be managed as one unit. You'll have to start the script yourself.
    SelfExtractingPwsh,
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
            #[cfg(unix)]
            InitSystem::SelfExtractingShell => "self-extracting-shell",
            InitSystem::SelfExtractingPwsh => "self-extracting-pwsh",
            #[cfg(target_os = "macos")]
            InitSystem::Launchd => "launchd",
        };
        write!(f, "{s}")
    }
}

/// Performs host_agent installation based on provided arguments.
///
/// Selects and invokes the appropriate init system installer or generates a script.
pub(crate) fn install_host_agent(arguments: &Args) -> Result<(), String> {
    let name = BINARY_NAME;
    #[cfg_attr(
        target_os = "windows",
        expect(unused_variables, reason = "windows doesn't need that, the others do")
    )]
    let bind_known_vals = |arg: &str| {
        bind_template_replacements(
            arg,
            env!("CARGO_PKG_DESCRIPTION"),
            &arguments.port.to_string(),
            &arguments.shutdown_command,
            &arguments.shared_secret,
        )
    };

    match arguments.init_system {
        #[cfg(target_os = "linux")]
        InitSystem::Systemd => {
            shuthost_common::systemd::install_self_as_service(
                name,
                &bind_known_vals(SYSTEMD_SERVICE_FILE_TEMPLATE),
            )?;
            shuthost_common::systemd::start_and_enable_self_as_service(name)?;
        }
        #[cfg(target_os = "linux")]
        InitSystem::OpenRC => {
            shuthost_common::openrc::install_self_as_service(
                name,
                &bind_known_vals(OPENRC_SERVICE_FILE_TEMPLATE),
            )?;
            shuthost_common::openrc::start_and_enable_self_as_service(name)?;
        }
        #[cfg(unix)]
        InitSystem::SelfExtractingShell => {
            let target_script_path = format!("./{name}_self_extracting");
            crate::install::self_extracting::generate_self_extracting_script(
                &[
                    ("SHUTHOST_SHARED_SECRET", &arguments.shared_secret),
                    ("PORT", &arguments.port.to_string()),
                    ("SHUTDOWN_COMMAND", &arguments.shutdown_command),
                ],
                &target_script_path,
            )?;
            // Start the self-extracting script in the background
            if let Err(e) = std::process::Command::new(&target_script_path).output() {
                eprintln!("Failed to start self-extracting script: {e}");
            } else {
                println!("Started self-extracting agent script in background.");
            }
        }
        InitSystem::SelfExtractingPwsh => {
            let target_script_path = format!("./{name}_self_extracting.ps1");
            crate::install::self_extracting::generate_self_extracting_ps1_script(
                &[
                    ("SHUTHOST_SHARED_SECRET", &arguments.shared_secret),
                    ("PORT", &arguments.port.to_string()),
                    ("SHUTDOWN_COMMAND", &arguments.shutdown_command),
                ],
                &target_script_path,
            )?;
            // Start the self-extracting script in the background
            let powershell_cmd = if cfg!(target_os = "windows") {
                "powershell.exe"
            } else {
                "pwsh"
            };
            if let Err(e) = std::process::Command::new(powershell_cmd)
                .arg("-File")
                .arg(&target_script_path)
                .output()
            {
                eprintln!("Failed to start self-extracting PowerShell script: {e}");
            } else {
                println!("Started self-extracting agent PowerShell script in background.");
            }
        }
        #[cfg(target_os = "macos")]
        InitSystem::Launchd => {
            shuthost_common::macos::install_self_as_service(
                name,
                &bind_known_vals(LAUNCHD_SERVICE_FILE_TEMPLATE),
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
    registration::print_registration_config(&registration::ServiceConfig {
        secret: arguments.shared_secret.clone(),
        port: arguments.port,
    });

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
pub(crate) fn get_inferred_init_system() -> InitSystem {
    #[cfg(target_os = "linux")]
    {
        if is_systemd() {
            InitSystem::Systemd
        } else if is_openrc() {
            InitSystem::OpenRC
        } else {
            InitSystem::SelfExtractingShell
        }
    }
    #[cfg(target_os = "macos")]
    {
        InitSystem::Launchd
    }
    #[cfg(target_os = "windows")]
    {
        InitSystem::SelfExtractingPwsh
    }
}

/// Attempts to determine the default network interface by parsing system routing information.
pub(crate) fn get_default_interface() -> Option<String> {
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
                    return line.split_whitespace().nth(4).map(|s| s.trim().to_string());
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

    #[cfg(target_os = "windows")]
    {
        let output = Command::new("powershell")
            .args(["-Command", "Get-NetRoute -DestinationPrefix 0.0.0.0/0 | Select-Object -First 1 -ExpandProperty InterfaceAlias"])
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !text.is_empty() { Some(text) } else { None }
    }
}

/// Retrieves the MAC address for the named network interface.
pub(crate) fn get_mac(interface: &str) -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        let output = Command::new("ip")
            .args(["link", "show", interface])
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
            if line.contains("ether") {
                return line.split_whitespace().nth(1).map(|s| s.to_string());
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

    #[cfg(target_os = "windows")]
    {
        let output = Command::new("powershell")
            .args(["-Command", &format!("Get-NetAdapter | Where-Object {{ $_.Name -eq '{}' }} | Select-Object -ExpandProperty MacAddress", interface)])
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !text.is_empty() { Some(text) } else { None }
    }
}

/// Retrieves the IP address for the named network interface.
pub(crate) fn get_ip(interface: &str) -> Option<String> {
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

    #[cfg(target_os = "windows")]
    {
        let output = Command::new("powershell")
            .args(["-Command", &format!("Get-NetIPAddress | Where-Object {{ $_.InterfaceAlias -eq '{}' -and $_.AddressFamily -eq 'IPv4' }} | Select-Object -First 1 -ExpandProperty IPAddress", interface)])
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !text.is_empty() { Some(text) } else { None }
    }
}

/// Retrieves the system hostname.
pub(crate) fn get_hostname() -> Option<String> {
    let output = Command::new("hostname").output().ok()?;

    let hostname = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if !hostname.is_empty() {
        // Return only the subdomain (first part before dot), matching client_installer behavior
        Some(hostname.split('.').next().unwrap_or(&hostname).to_string())
    } else {
        None
    }
}

/// Tests Wake-on-LAN packet reachability by listening and echoing back packets.
pub(crate) fn test_wol_reachability(port: u16) -> Result<(), String> {
    let socket = UdpSocket::bind(format!("0.0.0.0:{port}"))
        .map_err(|e| format!("Failed to bind test socket: {e}"))?;

    socket
        .set_broadcast(true)
        .map_err(|e| format!("Failed to set broadcast: {e}"))?;

    println!("Listening for WOL test packets on port {port}...");

    let mut buf = [0u8; 32];
    for _ in 0..2 {
        // Wait for both direct and broadcast tests
        if let Ok((_, addr)) = socket.recv_from(&mut buf) {
            // Echo back to confirm receipt
            socket
                .send_to(b"SHUTHOST_AGENT RECEIVED", addr)
                .map_err(|e| format!("Failed to send confirmation: {e}"))?;
        }
    }

    Ok(())
}
