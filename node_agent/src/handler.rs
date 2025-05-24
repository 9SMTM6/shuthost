use crate::server::ServiceArgs;
use shuthost_common::{is_timestamp_valid, parse_hmac_message, verify_hmac};

pub fn handle_request_without_shutdown(data: &[u8], config: &ServiceArgs) -> (String, bool) {
    let data_str = match std::str::from_utf8(data) {
        Ok(s) => s,
        Err(_) => return ("ERROR: Invalid UTF-8".to_string(), false),
    };

    let (timestamp, command, signature) = match parse_hmac_message(data_str) {
        Some(parts) => parts,
        None => return ("ERROR: Invalid request format".to_string(), false),
    };

    if !is_timestamp_valid(timestamp) {
        return ("ERROR: Timestamp out of range".to_string(), false);
    }

    let message = format!("{}|{}", timestamp, command);
    if !verify_hmac(&message, &signature, config.shared_secret.as_bytes()) {
        return ("ERROR: Invalid HMAC signature".to_string(), false);
    }

    (
        format!(
            "Now executing command: {}. Hopefully goodbye.",
            config.shutdown_command
        ),
        true,
    )
}

pub fn execute_shutdown(config: &ServiceArgs) -> Result<(), std::io::Error> {
    println!("Executing command: {}", &config.shutdown_command);
    std::process::Command::new("sh")
        .arg("-c")
        .arg(&config.shutdown_command)
        .spawn()?
        .wait()?;
    Ok(())
}
