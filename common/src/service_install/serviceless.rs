use std::{
    env,
    fs::{self, File},
    io::Write,
    os::unix::fs::PermissionsExt,
};

pub fn generate_self_extracting_script(
    secret: &str,
    port: u16,
    shutdown_command: &str,
    target_script_path: &str,
) -> Result<(), String> {
    let self_path = env::current_exe().map_err(|e| e.to_string())?;
    let self_binary = fs::read(&self_path).map_err(|e| e.to_string())?;

    let script_header = format!(
        r#"#!/bin/sh
SECRET="{secret}"
PORT="{port}"
SHUTDOWN_COMMAND="{shutdown_command}"

OUT=$(mktemp /tmp/selfbin.XXXXXX)
TAIL_LINE=$(awk '/^__BINARY_PAYLOAD_BELOW__/ {{ print NR + 1; exit 0; }}' "$0")
tail -n +$TAIL_LINE "$0" > "$OUT"
chmod +x "$OUT"
exec "$OUT" service --port="$PORT" --shutdown-command="$SHUTDOWN_COMMAND" --shared-secret="$SECRET" "$@"
exit 1

__BINARY_PAYLOAD_BELOW__
"#
    );

    let mut script = File::create(target_script_path).map_err(|e| e.to_string())?;
    script
        .write_all(script_header.as_bytes())
        .map_err(|e| e.to_string())?;
    script.write_all(&self_binary).map_err(|e| e.to_string())?;
    fs::set_permissions(target_script_path, fs::Permissions::from_mode(0o755))
        .map_err(|e| e.to_string())?;

    println!("Generated self-extracting script: {}", target_script_path);
    Ok(())
}
