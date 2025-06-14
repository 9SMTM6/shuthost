use std::{
    env,
    fs::{self, File},
    io::Write,
    os::unix::fs::PermissionsExt,
};

/// Generate a self-extracting script with arbitrary environment variables and execution command.
///
/// # Arguments
/// * `env_vars` - List of environment variables to include (name, value)
/// * `exec_args` - The command to run the extracted binary with (e.g., `"$OUT" foo --bar`)
/// * `target_script_path` - Path to write the resulting script to
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
        .map(|(k, v)| format!(r#"{k}="{v}""#))
        .collect::<Vec<_>>()
        .join("\n");

    let script_header = format!(
        r#"#!/bin/sh
{env_section}

OUT=$(mktemp /tmp/selfbin.XXXXXX)
TAIL_LINE=$(awk '/^__BINARY_PAYLOAD_BELOW__/ {{ print NR + 1; exit 0; }}' "$0")
tail -n +$TAIL_LINE "$0" > "$OUT"
chmod +x "$OUT"
{exec_args} "$@"
exit 1

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
