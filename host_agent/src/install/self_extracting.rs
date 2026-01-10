//! Generates a platform-agnostic self-extracting script embedding the current binary.
//!
//! Allows bundling the binary within a shell script with custom environment and execution.

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{
    env,
    fs::{self, File},
    io::Write,
};

use base64::{Engine as _, engine::general_purpose};

/// Generates a self-extracting shell script containing the current binary payload.
///
/// # Arguments
///
/// * `env_vars` - List of environment variable tuples (name, value) to include in the script.
/// * `target_script_path` - Destination path for the generated script file.
///
/// # Errors
///
/// Returns `Err` if any filesystem or I/O operations fail.
pub fn generate_self_extracting_script(
    env_vars: &[(&str, &str)],
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
nohup "$OUT" service --port="$PORT" --shutdown-command="$SHUTDOWN_COMMAND" "$@" >"$OUT.log" 2>&1 &
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
/// * `target_script_path` - Destination path for the generated script file (.ps1).
///
/// # Errors
///
/// Returns `Err` if any filesystem or I/O operations fail.
pub fn generate_self_extracting_ps1_script(
    env_vars: &[(&str, &str)],
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
$encodedBinary = $content.Substring($binaryStart)
$binaryBytes = [System.Convert]::FromBase64String($encodedBinary)
$tempFile = [System.IO.Path]::GetTempFileName()
[System.IO.File]::WriteAllBytes($tempFile, $binaryBytes)

# Make executable on Unix-like systems
if ($IsLinux -or $IsMacOS) {{
    & chmod +x $tempFile
}}

# Run the binary
$argList = @("service", "--port=$env:PORT", "--shutdown-command=$env:SHUTDOWN_COMMAND") + $args
Start-Process -FilePath $tempFile -ArgumentList $argList -RedirectStandardOutput "$tempFile.log" -RedirectStandardError "$tempFile.err"

exit 0

__BINARY_PAYLOAD_BELOW__
"#,
    );

    let mut script = File::create(target_script_path).map_err(|e| e.to_string())?;
    script
        .write_all(script_header.as_bytes())
        .map_err(|e| e.to_string())?;
    let encoded = general_purpose::STANDARD.encode(&self_binary);
    script.write_all(encoded.as_bytes()).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    fs::set_permissions(target_script_path, fs::Permissions::from_mode(0o750)).unwrap_or(());

    println!("Generated self-extracting PowerShell script: {target_script_path}");
    Ok(())
}
