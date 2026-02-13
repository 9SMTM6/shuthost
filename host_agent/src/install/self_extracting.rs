//! Generates a platform-agnostic self-extracting script embedding the current binary.
//!
//! Allows bundling the binary within a shell script with custom environment and execution.

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt as _;
use std::{
    env,
    fs::{self, File},
    io::Write as _,
};

use base64::{Engine as _, engine::general_purpose};
use shuthost_common::ResultMapErrExt as _;

/// Generates a self-extracting script from a template containing the current binary payload.
///
/// # Arguments
///
/// * `bound_template` - The script template string with placeholders already bound except for {encoded}.
/// * `target_script_path` - Destination path for the generated script file.
///
/// # Errors
///
/// Returns `Err` if any filesystem or I/O operations fail.
pub fn generate_self_extracting_script_from_template(
    bound_template: &str,
    target_script_path: &str,
) -> Result<(), String> {
    let self_path = env::current_exe().map_err_to_string_simple()?;
    let self_binary = fs::read(&self_path).map_err_to_string_simple()?;
    let encoded = general_purpose::STANDARD.encode(&self_binary);

    let script_content = bound_template.replace("{ encoded }", &encoded);

    let mut script = File::create(target_script_path).map_err_to_string_simple()?;
    script
        .write_all(script_content.as_bytes())
        .map_err_to_string_simple()?;
    #[cfg(unix)]
    fs::set_permissions(target_script_path, fs::Permissions::from_mode(0o750))
        .map_err_to_string_simple()?;

    println!("Generated self-extracting script: {target_script_path}");
    Ok(())
}
