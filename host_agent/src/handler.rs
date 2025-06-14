use crate::server::ServiceArgs;
use shuthost_common::{is_timestamp_in_valid_range, parse_hmac_message, verify_hmac};

pub fn handle_request_without_shutdown(
    data: &[u8],
    config: &ServiceArgs,
    peer_addr: &str,
) -> (String, bool) {
    let data_str = match std::str::from_utf8(data) {
        Ok(s) => s,
        Err(_) => {
            eprintln!("Invalid UTF-8 in request from {}: {:?}", peer_addr, data);
            return ("ERROR: Invalid UTF-8".to_string(), false);
        }
    };

    let (timestamp, command, signature) = match parse_hmac_message(data_str) {
        Some(parts) => parts,
        None => {
            eprintln!("Invalid request format from {}: {}", peer_addr, data_str);
            return ("ERROR: Invalid request format".to_string(), false);
        }
    };

    if !is_timestamp_in_valid_range(timestamp) {
        eprintln!("Timestamp out of range from {}: {}", peer_addr, timestamp);
        return ("ERROR: Timestamp out of range".to_string(), false);
    }

    let message = format!("{}|{}", timestamp, command);
    if !verify_hmac(&message, &signature, &config.shared_secret) {
        eprintln!(
            "Invalid HMAC signature from {} for message: '{}', signature: '{}'",
            peer_addr, message, signature
        );
        return ("ERROR: Invalid HMAC signature".to_string(), false);
    }

    // Special handling for status check command
    if command == "status" {
        return ("OK: status".to_string(), false);
    }

    // Only allow the expected shutdown command
    if command != "shutdown" {
        eprintln!("Invalid command from {}: {}", peer_addr, command);
        return ("ERROR: Invalid command".to_string(), false);
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
