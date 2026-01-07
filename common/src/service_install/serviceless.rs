//! Generates a platform-agnostic self-extracting script embedding the current binary.
//!
//! Allows bundling the binary within a shell script with custom environment and execution.

use std::{
    env,
    fs::{self, File},
    io::Write,
};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

/// Generates a self-extracting shell script containing the current binary payload.
///
/// # Arguments
///
/// * `env_vars` - List of environment variable tuples (name, value) to include in the script.
/// * `exec_args` - Shell command arguments for the extracted binary.
/// * `target_script_path` - Destination path for the generated script file.
///
/// # Errors
///
/// Returns `Err` if any filesystem or I/O operations fail.
pub fn generate_self_extracting_script(
    env_vars: &[(&str, &str)],
    exec_args: &str,
    target_script_path: &str,
) -> Result<(), String> {
    let self_path = env::current_exe().map_err(|e| e.to_string())?;
    let self_binary = fs::read(&self_path).map_err(|e| e.to_string())?;

    // Format environment variable declarations
    let env_section = env_vars
        .iter()
        .map(|&(k, v)| format!(r#"export {k}="{v}""#))
        .collect::<Vec<_>>()
        .join("\n");

    let script_header = format!(
        r#"#!/bin/sh
{env_section}

OUT=$(mktemp /tmp/selfbin.XXXXXX)
TAIL_LINE=$(awk '/^__BINARY_PAYLOAD_BELOW__/ {{ print NR + 1; exit 0; }}' "$0")
tail -n +$TAIL_LINE "$0" > "$OUT"
chmod +x "$OUT"
nohup "$OUT" {exec_args} "$@" >"$OUT.log" 2>&1 &
exit 0

__BINARY_PAYLOAD_BELOW__
"#
    );

    let mut script = File::create(target_script_path).map_err(|e| e.to_string())?;
    script
        .write_all(script_header.as_bytes())
        .map_err(|e| e.to_string())?;
    script.write_all(&self_binary).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    fs::set_permissions(target_script_path, fs::Permissions::from_mode(0o750))
        .map_err(|e| e.to_string())?;

    println!("Generated self-extracting script: {target_script_path}");
    Ok(())
}

/// Generates a self-extracting PowerShell script containing the current binary payload.
/// Compatible with PowerShell on Windows and Linux.
///
/// # Arguments
///
/// * `env_vars` - List of environment variable tuples (name, value) to include in the script.
/// * `exec_args` - Space-separated command arguments for the extracted binary.
/// * `target_script_path` - Destination path for the generated script file (.ps1).
///
/// # Errors
///
/// Returns `Err` if any filesystem or I/O operations fail.
pub fn generate_self_extracting_ps1_script(
    env_vars: &[(&str, &str)],
    exec_args: &str,
    target_script_path: &str,
) -> Result<(), String> {
    let self_path = env::current_exe().map_err(|e| e.to_string())?;
    let self_binary = fs::read(&self_path).map_err(|e| e.to_string())?;

    // Format environment variable declarations
    let env_section = env_vars
        .iter()
        .map(|&(k, v)| format!(r#"$env:{k} = "{v}""#))
        .collect::<Vec<_>>()
        .join("\n");

    // Prepare argument list for PowerShell array
    let arg_list_str = exec_args
        .split_whitespace()
        .map(|s| format!("'{}'", s))
        .collect::<Vec<_>>()
        .join(", ");

    let script_header = format!(
        r#"#!/usr/bin/env pwsh
# Set environment variables
{env_section}

# Extract and run the binary
$scriptPath = $MyInvocation.MyCommand.Path
$content = Get-Content $scriptPath -Raw
$marker = "__BINARY_PAYLOAD_BELOW__`n"
$markerIndex = $content.IndexOf($marker)
$binaryStart = $markerIndex + $marker.Length
$allBytes = [System.IO.File]::ReadAllBytes($scriptPath)
$binaryBytes = $allBytes[$binaryStart..($allBytes.Length - 1)]
$tempFile = [System.IO.Path]::GetTempFileName()
[System.IO.File]::WriteAllBytes($tempFile, $binaryBytes)

# Make executable on Unix-like systems
if ($IsLinux -or $IsMacOS) {{
    & chmod +x $tempFile
}}

# Run the binary
$argList = @({arg_list_str}) + $args
Start-Process -NoWait -FilePath $tempFile -ArgumentList $argList -RedirectStandardOutput "$tempFile.log" -RedirectStandardError "$tempFile.log"

exit 0

__BINARY_PAYLOAD_BELOW__
"#,
    );

    let mut script = File::create(target_script_path).map_err(|e| e.to_string())?;
    script
        .write_all(script_header.as_bytes())
        .map_err(|e| e.to_string())?;
    script.write_all(&self_binary).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    fs::set_permissions(target_script_path, fs::Permissions::from_mode(0o750)).unwrap_or(());

    println!("Generated self-extracting PowerShell script: {target_script_path}");
    Ok(())
}
