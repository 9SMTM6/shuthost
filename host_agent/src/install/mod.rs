//! Installation and runtime utilities for the `host_agent` binary.
//!
//! Handles subcommand parsing, agent installation across init systems, network interface discovery, and Wake-on-LAN testing.

pub mod self_extracting;

use alloc::string;
use core::iter;
use std::process::Command;

use clap::{Parser, ValueEnum as _};
use core::fmt;
use rand::{RngExt as _, distr, rng};

#[cfg(target_os = "linux")]
use shuthost_common::{is_openrc, is_systemd};

use crate::{DEFAULT_PORT, registration, server::get_default_shutdown_command};

/// The binary name, derived from the Cargo package name.
pub(super) const BINARY_NAME: &str = env!("CARGO_PKG_NAME");
#[cfg(any(target_os = "linux", test))]
pub(crate) const SYSTEMD_SERVICE_FILE_TEMPLATE: &str =
    include_str!("shuthost_host_agent.service.tmpl.ini");
#[cfg(any(target_os = "macos", test))]
pub(crate) const LAUNCHD_SERVICE_FILE_TEMPLATE: &str =
    include_str!("com.github_9smtm6.shuthost_host_agent.plist.tmpl.xml");
#[cfg(any(target_os = "linux", test))]
pub(crate) const OPENRC_SERVICE_FILE_TEMPLATE: &str =
    include_str!("openrc.shuthost_host_agent.tmpl.sh");
#[cfg(unix)]
pub(crate) const SELF_EXTRACTING_SHELL_TEMPLATE: &str = include_str!("self_extracting.tmpl.sh");
pub(crate) const SELF_EXTRACTING_PWSH_TEMPLATE: &str = include_str!("self_extracting.tmpl.ps1");

/// Generates a random secret string suitable for use as an HMAC key.
///
/// Returns a 32-character alphanumeric string.
#[must_use]
pub fn generate_secret() -> String {
    // Simple random secret generation: 32 characters
    let mut rng = rng();
    iter::repeat_with(|| rng.sample(distr::Alphanumeric) as char)
        .take(32)
        .collect()
}

/// Binds template placeholders with actual values.
pub(crate) fn bind_template_replacements(
    template: &str,
    description: &str,
    port: u16,
    shutdown_command: &str,
    secret: &str,
    hostname: &str,
) -> String {
    template
        .replace("{ description }", description)
        .replace("{ port }", &port.to_string())
        .replace("{ shutdown_command }", shutdown_command)
        .replace("{ secret }", secret)
        .replace("{ name }", BINARY_NAME)
        .replace("{ hostname }", hostname)
}

/// Arguments for the `install` subcommand of `host_agent`.
#[derive(Debug, Parser)]
pub struct Args {
    #[arg(long, short, default_value_t = DEFAULT_PORT)]
    pub port: u16,

    #[arg(long, short = 'c', default_value_t = get_default_shutdown_command())]
    pub shutdown_command: String,

    #[arg(long,  short, default_value_t = generate_secret())]
    pub shared_secret: String,

    #[arg(long, short, default_value_t = get_inferred_init_system())]
    pub init_system: InitSystem,

    #[arg(long, short = 'n', default_value_t = default_hostname())]
    pub hostname: String,
}

/// Supported init systems for installing the `host_agent`.
#[derive(Debug, Clone, Copy, clap::ValueEnum, PartialEq, Eq)]
pub enum InitSystem {
    /// Systemd init system (Linux).
    #[cfg(target_os = "linux")]
    Systemd,
    /// `OpenRC` init system (Linux).
    #[cfg(target_os = "linux")]
    #[clap(name = "openrc")]
    OpenRC,
    /// Generates a self-extracting shell script that embeds the compiled binary. The purpose is to keep the configuration readable (and editable) while being a single file that can be managed as one unit. You'll have to start the script yourself. [aliases: sh]
    #[cfg(unix)]
    #[clap(alias = "sh")]
    SelfExtractingShell,
    /// Generates a self-extracting `PowerShell` script that embeds the compiled binary. The purpose is to keep the configuration readable (and editable) while being a single file that can be managed as one unit. Note: Unlike the shell variant, the `PowerShell` script runs attached to the service process and does not automatically background itself. The installer will spawn the script in the background. [aliases: pwsh]
    #[clap(alias = "pwsh")]
    SelfExtractingPwsh,
    /// Launchd init system (macOS).
    #[cfg(target_os = "macos")]
    Launchd,
}

impl fmt::Display for InitSystem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            self.to_possible_value()
                .expect("No skipped variants")
                .get_name()
        )
    }
}

/// Performs `host_agent` installation based on provided arguments.
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
            arguments.port,
            &arguments.shutdown_command,
            &arguments.shared_secret,
            &arguments.hostname,
        )
    };

    match arguments.init_system {
        #[cfg(target_os = "linux")]
        InitSystem::Systemd => install_systemd(name, bind_known_vals)?,
        #[cfg(target_os = "linux")]
        InitSystem::OpenRC => install_openrc(name, bind_known_vals)?,
        #[cfg(unix)]
        InitSystem::SelfExtractingShell => install_self_extracting_shell(name, bind_known_vals)?,
        InitSystem::SelfExtractingPwsh => {
            install_self_extracting_pwsh(name, arguments, bind_known_vals)?;
        }
        #[cfg(target_os = "macos")]
        InitSystem::Launchd => install_launchd(name, &bind_known_vals)?,
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
        hostname: arguments.hostname.clone(),
    });

    Ok(())
}

#[cfg(target_os = "linux")]
fn install_systemd(name: &str, bind_known_vals: impl Fn(&str) -> String) -> Result<(), String> {
    shuthost_common::systemd::install_self_as_service(
        name,
        &bind_known_vals(SYSTEMD_SERVICE_FILE_TEMPLATE),
    )?;
    shuthost_common::systemd::start_and_enable_self_as_service(name)?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn install_openrc(name: &str, bind_known_vals: impl Fn(&str) -> String) -> Result<(), String> {
    shuthost_common::openrc::install_self_as_service(
        name,
        &bind_known_vals(OPENRC_SERVICE_FILE_TEMPLATE),
    )?;
    shuthost_common::openrc::start_and_enable_self_as_service(name)?;
    Ok(())
}

#[cfg(unix)]
fn install_self_extracting_shell(
    name: &str,
    bind_known_vals: impl Fn(&str) -> String,
) -> Result<(), String> {
    let target_script_path = format!("./{name}_self_extracting");
    self_extracting::generate_self_extracting_script_from_template(
        &bind_known_vals(SELF_EXTRACTING_SHELL_TEMPLATE),
        &target_script_path,
    )?;
    // Start the self-extracting script in the background
    if let Err(e) = Command::new(&target_script_path).output() {
        eprintln!("Failed to start self-extracting script: {e}");
    } else {
        println!("Started self-extracting agent script in background.");
    }
    Ok(())
}

fn install_self_extracting_pwsh(
    name: &str,
    #[cfg_attr(
        not(target_os = "windows"),
        expect(unused_variables, reason = "only unused on non-windows")
    )]
    arguments: &Args,
    bind_known_vals: impl Fn(&str) -> String,
) -> Result<(), String> {
    let target_script_path = format!("./{name}_self_extracting.ps1");
    self_extracting::generate_self_extracting_script_from_template(
        &bind_known_vals(SELF_EXTRACTING_PWSH_TEMPLATE),
        &target_script_path,
    )?;
    let powershell_cmd = if cfg!(target_os = "windows") {
        "powershell.exe"
    } else {
        "pwsh"
    };

    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            let exe_path = std::path::Path::new(&appdata)
                .join("shuthost")
                .join("host_agent.exe");
            let exe_path_str = exe_path.to_string_lossy();
            let ps_command = format!(
                "$ruleName = \"ShutHost Host Agent\"; $existingRule = Get-NetFirewallRule -DisplayName $ruleName -ErrorAction SilentlyContinue; if (-not $existingRule) {{ New-NetFirewallRule -DisplayName $ruleName -Direction Inbound -Protocol TCP -LocalPort {} -Program \"{}\" -Action Allow -Profile Any }}",
                arguments.port,
                exe_path_str.replace('\\', "\\\\").replace('"', "\\\"")
            );
            if let Err(e) = Command::new(powershell_cmd)
                .arg("-Command")
                .arg(&ps_command)
                .output()
            {
                eprintln!("Failed to add Windows Firewall rule: {e}");
            }
        }
    }

    // Start the PowerShell script in the background
    // Unlike the shell script, the PowerShell script doesn't self-background,
    // so we need to background it here by spawning without waiting

    if let Err(e) = Command::new(powershell_cmd)
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-File")
        .arg(&target_script_path)
        .spawn()
    {
        eprintln!("Failed to start self-extracting PowerShell script: {e}");
    } else {
        println!("Started self-extracting agent PowerShell script in background.");
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn install_launchd(name: &str, bind_known_vals: impl Fn(&str) -> String) -> Result<(), String> {
    shuthost_common::macos::install_self_as_service(
        name,
        &bind_known_vals(LAUNCHD_SERVICE_FILE_TEMPLATE),
    )?;
    shuthost_common::macos::start_and_enable_self_as_service(name)?;
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
                return line
                    .split_whitespace()
                    .nth(1)
                    .map(string::ToString::to_string);
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
                    .map(ToString::to_string);
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

    if hostname.is_empty() {
        None
    } else {
        // Return only the subdomain (first part before dot), matching client_installer behavior
        Some(hostname.split('.').next().unwrap_or(&hostname).to_string())
    }
}

/// Returns the default hostname, using the system hostname if available, otherwise "unknown".
pub(crate) fn default_hostname() -> String {
    get_hostname().unwrap_or_else(|| "unknown".to_string())
}

/// Tests Wake-on-LAN packet reachability by listening and echoing back packets.
pub(crate) fn test_wol_reachability(port: u16) -> Result<(), String> {
    let socket = shuthost_common::create_broadcast_socket(port)?;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_secret() {
        let secret = generate_secret();
        assert_eq!(secret.len(), 32);
    }
}
