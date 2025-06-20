//! Generates a platform-agnostic self-extracting script embedding the current binary.
//!
//! Allows bundling the binary within a shell script with custom environment and execution.

use std::{
    env,
    fs::{self, File},
    io::Write,
    os::unix::fs::PermissionsExt,
};

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
    fs::set_permissions(target_script_path, fs::Permissions::from_mode(0o750))
        .map_err(|e| e.to_string())?;

    println!("Generated self-extracting script: {}", target_script_path);
    Ok(())
}
