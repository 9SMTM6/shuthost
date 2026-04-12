//! Installation and runtime utilities for the `host_agent` binary.
//!
//! Handles subcommand parsing, agent installation across init systems, network interface discovery, and Wake-on-LAN testing.

pub mod self_extracting;

use core::iter;
use std::{path::Path, process::Command};

use clap::{Parser, ValueEnum as _};
use core::fmt;
use rand::{RngExt as _, distr, rng};

#[cfg(target_os = "linux")]
use shuthost_common::{is_openrc, is_systemd};

use crate::{registration, server::get_default_shutdown_command};

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
    broadcast_port: u16,
    shutdown_command: &str,
    secret: &str,
    hostname: &str,
) -> String {
    template
        .replace("{ description }", description)
        .replace("{ port }", &port.to_string())
        .replace("{ broadcast_port }", &broadcast_port.to_string())
        .replace("{ shutdown_command }", shutdown_command)
        .replace("{ secret }", secret)
        .replace("{ name }", BINARY_NAME)
        .replace("{ hostname }", hostname)
}

// TODO: update command needs integration tests

/// Arguments for the `install` subcommand of `host_agent`.
#[derive(Debug, Parser)]
pub struct Args {
    #[arg(long, short, default_value_t = shuthost_common::DEFAULT_AGENT_TCP_PORT)]
    pub port: u16,

    #[arg(long, short = 'b', default_value_t = shuthost_common::DEFAULT_COORDINATOR_BROADCAST_PORT)]
    pub broadcast_port: u16,

    #[arg(long, short = 'c', default_value_t = get_default_shutdown_command())]
    pub shutdown_command: String,

    #[arg(long, short, default_value_t = generate_secret())]
    pub shared_secret: String,

    #[arg(long, short, default_value_t = get_inferred_init_system())]
    pub init_system: InitSystem,

    #[arg(long, short = 'n', default_value_t = default_hostname())]
    pub hostname: String,
}

/// Arguments for the `update` subcommand of `host_agent`.
#[derive(Debug, Parser)]
pub struct UpdateArgs {
    /// Path to a self-extracting script. When provided, the update command skips
    /// init-system autodetection and updates this script directly.
    #[arg(long, short = 'p')]
    pub script_path: Option<String>,
}

/// Supported init systems for installing the `host_agent`.
#[derive(Debug, Clone, Copy, clap::ValueEnum, PartialEq, Eq)]
pub enum InitSystem {
    /// Systemd init system (Linux).
    #[cfg_attr(not(target_os = "linux"), clap(skip))]
    Systemd,
    /// `OpenRC` init system (Linux).
    #[cfg_attr(not(target_os = "linux"), clap(skip))]
    #[clap(name = "openrc")]
    OpenRC,
    /// Generates a self-extracting shell script that embeds the compiled binary. The purpose is to keep the configuration readable (and editable) while being a single file that can be managed as one unit. You'll have to start the script yourself. [aliases: sh]
    #[cfg_attr(not(unix), clap(skip))]
    #[cfg_attr(unix, clap(alias = "sh"))]
    SelfExtractingShell,
    /// Generates a self-extracting `PowerShell` script that embeds the compiled binary. The purpose is to keep the configuration readable (and editable) while being a single file that can be managed as one unit. Note: Unlike the shell variant, the `PowerShell` script runs attached to the service process and does not automatically background itself. The installer will spawn the script in the background. [aliases: pwsh]
    #[clap(alias = "pwsh")]
    SelfExtractingPwsh,
    /// Launchd init system (macOS).
    #[cfg_attr(not(target_os = "macos"), clap(skip))]
    Launchd,
}

impl fmt::Display for InitSystem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            self.to_possible_value()
                .expect("Invoking this with a skipped variant, creating skipped variants in the skipped OSs is forbidden.")
                .get_name()
        )
    }
}

impl From<InitSystem> for shuthost_common::InitSystem {
    fn from(v: InitSystem) -> shuthost_common::InitSystem {
        use InitSystem as tIS;
        use shuthost_common::InitSystem as cIS;
        match v {
            tIS::Systemd => cIS::Systemd,
            tIS::OpenRC => cIS::OpenRC,
            tIS::Launchd => cIS::Launchd,
            tIS::SelfExtractingShell => cIS::SelfExtractingShell,
            tIS::SelfExtractingPwsh => cIS::SelfExtractingPwsh,
        }
    }
}

impl From<shuthost_common::InitSystem> for InitSystem {
    fn from(v: shuthost_common::InitSystem) -> InitSystem {
        use InitSystem as tIS;
        use shuthost_common::InitSystem as cIS;
        match v {
            cIS::Systemd => tIS::Systemd,
            cIS::OpenRC => tIS::OpenRC,
            cIS::Launchd => tIS::Launchd,
            cIS::SelfExtractingShell => tIS::SelfExtractingShell,
            cIS::SelfExtractingPwsh => tIS::SelfExtractingPwsh,
        }
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
            arguments.broadcast_port,
            &arguments.shutdown_command,
            &arguments.shared_secret,
            &arguments.hostname,
        )
    };

    match arguments.init_system {
        InitSystem::Systemd => {
            #[cfg(target_os = "linux")]
            install_systemd(name, bind_known_vals)?;
            #[cfg(not(target_os = "linux"))]
            unreachable!("Systemd is not supported on this platform");
        }
        InitSystem::OpenRC => {
            #[cfg(target_os = "linux")]
            install_openrc(name, bind_known_vals)?;
            #[cfg(not(target_os = "linux"))]
            unreachable!("OpenRC is not supported on this platform");
        }
        InitSystem::SelfExtractingShell => {
            #[cfg(unix)]
            install_self_extracting_shell(name, bind_known_vals)?;
            #[cfg(not(unix))]
            unreachable!("Self-extracting shell installs are not supported on this platform");
        }
        InitSystem::SelfExtractingPwsh => {
            install_self_extracting_pwsh(name, arguments, bind_known_vals)?;
        }
        InitSystem::Launchd => {
            #[cfg(target_os = "macos")]
            install_launchd(name, &bind_known_vals)?;
            #[cfg(not(target_os = "macos"))]
            unreachable!("Launchd is not supported on this platform");
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
        broadcast_port: arguments.broadcast_port,
        hostname: arguments.hostname.clone(),
        shutdown_command: arguments.shutdown_command.clone(),
    });

    Ok(())
}

/// Updates an existing installation in place using the current installed config.
///
/// This command does not support switching the init system during updates.
///
/// For self-extracting installs, the update command currently detects the generated
/// script in the local working directory and regenerates that same script.
pub(crate) fn update_host_agent(args: &UpdateArgs) -> Result<(), String> {
    let name = BINARY_NAME;

    let init_system = if let Some(script_path) = args.script_path.as_deref() {
        if Path::new(script_path)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("ps1"))
            || Path::new(script_path)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("PS1"))
        {
            InitSystem::SelfExtractingPwsh
        } else {
            InitSystem::SelfExtractingShell
        }
    } else {
        registration::detect_installation_init_system()?
    };

    let script_path = args.script_path.as_deref();

    match init_system {
        InitSystem::Systemd => {
            #[cfg(target_os = "linux")]
            update_systemd(name)?;
            #[cfg(not(target_os = "linux"))]
            unreachable!("Systemd updates are not supported on this platform");
        }
        InitSystem::OpenRC => {
            #[cfg(target_os = "linux")]
            update_openrc(name)?;
            #[cfg(not(target_os = "linux"))]
            unreachable!("OpenRC updates are not supported on this platform");
        }
        InitSystem::SelfExtractingShell => {
            #[cfg(unix)]
            update_self_extracting_shell(name, script_path)?;
            #[cfg(not(unix))]
            unreachable!("Self-extracting shell updates are not supported on this platform");
        }
        InitSystem::SelfExtractingPwsh => update_self_extracting_pwsh(name, script_path)?,
        InitSystem::Launchd => {
            #[cfg(target_os = "macos")]
            update_launchd(name)?;
            #[cfg(not(target_os = "macos"))]
            unreachable!("Launchd updates are not supported on this platform");
        }
    }

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

#[cfg(target_os = "linux")]
fn update_systemd(name: &str) -> Result<(), String> {
    let config = registration::parse_config(&registration::Args {
        init_system: InitSystem::Systemd,
        script_path: None,
    })?;

    let bind_known_vals = |arg: &str| {
        bind_template_replacements(
            arg,
            env!("CARGO_PKG_DESCRIPTION"),
            config.port,
            config.broadcast_port,
            &config.shutdown_command,
            &config.secret,
            &config.hostname,
        )
    };

    shuthost_common::systemd::install_self_as_service(
        name,
        &bind_known_vals(SYSTEMD_SERVICE_FILE_TEMPLATE),
    )?;
    shuthost_common::systemd::start_and_enable_self_as_service(name)?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn update_openrc(name: &str) -> Result<(), String> {
    let config = registration::parse_config(&registration::Args {
        init_system: InitSystem::OpenRC,
        script_path: None,
    })?;

    let bind_known_vals = |arg: &str| {
        bind_template_replacements(
            arg,
            env!("CARGO_PKG_DESCRIPTION"),
            config.port,
            config.broadcast_port,
            &config.shutdown_command,
            &config.secret,
            &config.hostname,
        )
    };

    shuthost_common::openrc::install_self_as_service(
        name,
        &bind_known_vals(OPENRC_SERVICE_FILE_TEMPLATE),
    )?;
    shuthost_common::openrc::start_and_enable_self_as_service(name)?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn update_launchd(name: &str) -> Result<(), String> {
    let config = registration::parse_config(&registration::Args {
        init_system: InitSystem::Launchd,
        script_path: None,
    })?;

    let bind_known_vals = |arg: &str| {
        bind_template_replacements(
            arg,
            env!("CARGO_PKG_DESCRIPTION"),
            config.port,
            config.broadcast_port,
            &config.shutdown_command,
            &config.secret,
            &config.hostname,
        )
    };

    shuthost_common::macos::install_self_as_service(
        name,
        &bind_known_vals(LAUNCHD_SERVICE_FILE_TEMPLATE),
    )?;
    shuthost_common::macos::start_and_enable_self_as_service(name)?;
    Ok(())
}

#[cfg(unix)]
fn update_self_extracting_shell(name: &str, script_path: Option<&str>) -> Result<(), String> {
    let path = script_path.map_or_else(|| format!("./{name}_self_extracting"), ToString::to_string);

    let config = registration::parse_config(&registration::Args {
        init_system: InitSystem::SelfExtractingShell,
        script_path: Some(path.clone()),
    })?;

    let bind_known_vals = |arg: &str| {
        bind_template_replacements(
            arg,
            env!("CARGO_PKG_DESCRIPTION"),
            config.port,
            config.broadcast_port,
            &config.shutdown_command,
            &config.secret,
            &config.hostname,
        )
    };

    self_extracting::generate_self_extracting_script_from_template(
        &bind_known_vals(SELF_EXTRACTING_SHELL_TEMPLATE),
        &path,
    )?;

    if let Err(e) = Command::new(&path).output() {
        eprintln!("Failed to start updated self-extracting script: {e}");
    } else {
        println!("Started updated self-extracting agent script in background.");
    }

    Ok(())
}

fn update_self_extracting_pwsh(name: &str, script_path: Option<&str>) -> Result<(), String> {
    let path = script_path.map_or_else(
        || format!("./{name}_self_extracting.ps1"),
        ToString::to_string,
    );

    let config = registration::parse_config(&registration::Args {
        init_system: InitSystem::SelfExtractingPwsh,
        script_path: Some(path.clone()),
    })?;

    let bind_known_vals = |arg: &str| {
        bind_template_replacements(
            arg,
            env!("CARGO_PKG_DESCRIPTION"),
            config.port,
            config.broadcast_port,
            &config.shutdown_command,
            &config.secret,
            &config.hostname,
        )
    };

    self_extracting::generate_self_extracting_script_from_template(
        &bind_known_vals(SELF_EXTRACTING_PWSH_TEMPLATE),
        &path,
    )?;

    let powershell_cmd = if cfg!(target_os = "windows") {
        "powershell.exe"
    } else {
        "pwsh"
    };

    if let Err(e) = Command::new(powershell_cmd)
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-File")
        .arg(&path)
        .spawn()
    {
        eprintln!("Failed to start updated self-extracting PowerShell script: {e}");
    } else {
        println!("Started updated self-extracting agent PowerShell script in background.");
    }

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
                return line.split_whitespace().nth(1).map(ToString::to_string);
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
    fn generate_secret_works() {
        let secret = generate_secret();
        assert_eq!(secret.len(), 32);
    }
}
