use std::{fmt::Display, path::PathBuf, str::FromStr};

use crate::{
    install::{
        InitSystem, get_default_interface, get_hostname, get_inferred_init_system, get_ip, get_mac,
    },
    registration::{self, parse_config},
};
use shuthost_common::{ResultMapErrExt, UnwrapToStringExt};

#[derive(Debug, Clone)]
pub struct LossyPath(PathBuf);

impl FromStr for LossyPath {
    type Err = <PathBuf as FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(PathBuf::from_str(s)?))
    }
}

impl Display for LossyPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.to_string_lossy())
    }
}

#[derive(Debug)]
pub struct ControlScriptValues {
    pub host_ip: String,
    pub port: u16,
    pub shared_secret: String,
    pub mac_address: String,
    pub hostname: String,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum ScriptType {
    /// A .sh script for macOS/Linux/Unix hosts
    UnixShell,
    /// A .ps1 PowerShell script (should support all platforms with PowerShell installed)
    Pwsh,
}

impl std::fmt::Display for ScriptType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            ScriptType::UnixShell => write!(f, "unix-shell"),
            ScriptType::Pwsh => write!(f, "pwsh"),
        }
    }
}

const fn get_default_script_type() -> ScriptType {
    #[cfg(target_os = "windows")]
    {
        ScriptType::Pwsh
    }
    #[cfg(not(target_os = "windows"))]
    {
        ScriptType::UnixShell
    }
}

pub fn generate_control_script_from_values(
    raw: &'static str,
    values: &ControlScriptValues,
) -> String {
    raw.replace("{host_ip}", &values.host_ip)
        .replace("{port}", &values.port.to_string())
        .replace("{shared_secret}", &values.shared_secret)
        .replace("{mac_address}", &values.mac_address)
        .replace("{hostname}", &values.hostname)
}

fn get_default_output_path() -> LossyPath {
    let hostname = get_hostname().unwrap_or_to_string("unknown");
    LossyPath(PathBuf::from(format!(
        "shuthost_direct_control_{}",
        hostname
    )))
}

#[derive(Debug, clap::Parser)]
pub struct Args {
    /// Output path for the generated control script.
    /// Powershell scripts will have a .ps1 extension automatically added.
    #[arg(long = "output", short = 'o', default_value_t = get_default_output_path())]
    pub output: LossyPath,

    /// The init system used by the host_agent installation.
    #[arg(long = "init-system", default_value_t = get_inferred_init_system())]
    pub init_system: InitSystem,

    /// Type of the script to generate.
    #[arg(long = "type", default_value_t = get_default_script_type())]
    pub script_type: ScriptType,

    /// Path to the self-extracting script, only used if init-system is `self-extracting-*`.
    #[arg(long = "script-path")]
    pub script_path: Option<String>,
}

pub(crate) fn generate_control_script(
    init_system: InitSystem,
    script_path: Option<&str>,
    script_type: ScriptType,
) -> Result<String, String> {
    let config = parse_config(&registration::Args {
        init_system,
        script_path: script_path.map(|s| s.to_string()),
    })?;

    let (ip, mac) = if let Some(interface) = get_default_interface() {
        let ip = get_ip(&interface).unwrap_or_to_string("127.0.0.1");
        let mac = get_mac(&interface).unwrap_or_to_string("00:00:00:00:00:00");
        (ip, mac)
    } else {
        eprintln!(
            "Failed to determine the default network interface. Assuming test environment and using localhost and dummy MAC for script generation."
        );
        ("127.0.0.1".to_string(), "00:00:00:00:00:00".to_string())
    };
    let hostname = get_hostname().unwrap_or_to_string("unknown");

    let values = ControlScriptValues {
        host_ip: ip,
        port: config.port,
        shared_secret: config.secret,
        mac_address: mac,
        hostname,
    };

    Ok(match script_type {
        ScriptType::UnixShell => generate_control_script_from_values(
            include_str!("../../scripts/enduser_templates/direct_control.tmpl.sh"),
            &values,
        ),
        ScriptType::Pwsh => generate_control_script_from_values(
            include_str!("../../scripts/enduser_templates/direct_control.tmpl.ps1"),
            &values,
        ),
    })
}

pub(crate) fn write_control_script(args: &Args) -> Result<(), String> {
    let script = generate_control_script(
        args.init_system,
        args.script_path.as_deref(),
        args.script_type,
    )?;

    let mut output_path = args.output.0.clone();
    if matches!(args.script_type, ScriptType::Pwsh) {
        output_path.set_extension("ps1");
    }

    std::fs::write(&output_path, &script).map_err_to_string(&format!(
        "Failed to write script to {}",
        output_path.display()
    ))?;

    // Make executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&output_path)
            .map_err_to_string("Failed to get metadata")?
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&output_path, perms)
            .map_err_to_string("Failed to set permissions")?;
    }

    println!("Control script generated at: {}", output_path.display());
    Ok(())
}
