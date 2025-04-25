use std::time::{SystemTime, UNIX_EPOCH};
use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::server::ServiceArgs;

const ALLOWED_WINDOW: u64 = 30; // Seconds

pub fn handle_request(data: &[u8], config: &ServiceArgs) -> String {
    let data_str = match std::str::from_utf8(data) {
        Ok(s) => s,
        Err(_) => return "ERROR: Invalid UTF-8".to_string(),
    };

    let parts: Vec<&str> = data_str.split('|').collect();
    if parts.len() != 3 {
        return "ERROR: Invalid request format".to_string();
    }

    let (timestamp_str, command, signature) = (parts[0], parts[1], parts[2]);
    
    // Step 1: Verify timestamp is within the allowed window
    let timestamp: u64 = match timestamp_str.parse() {
        Ok(ts) => ts,
        Err(_) => return "ERROR: Invalid timestamp".to_string(),
    };

    if !is_timestamp_valid(timestamp) {
        return "ERROR: Timestamp out of range".to_string();
    }

    // Step 2: Verify the HMAC signature
    let message = format!("{}|{}", timestamp_str, command);
    if !verify_hmac(&message, signature, config.shared_secret.as_bytes()) {
        return "ERROR: Invalid HMAC signature".to_string();
    }

    // Step 3: Handle the shutdown or sleep command
    match execute_command(&config.shutdown_command) {
        Ok(_) => format!("Successfully executed command: {}", config.shutdown_command),
        Err(e) => format!("ERROR: Failed to execute command: {}", e),
    }
}

// Step 4: Check if the timestamp is within the allowed window
fn is_timestamp_valid(timestamp: u64) -> bool {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    now.abs_diff(timestamp) <= ALLOWED_WINDOW
}

// Step 5: Verify HMAC signature
fn verify_hmac(message: &str, received_signature: &str, secret: &[u8]) -> bool {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret)
        .expect("HMAC can take a key of any size");
    mac.update(message.as_bytes());
    let computed_signature = mac.finalize().into_bytes();
    let computed_signature_hex = hex::encode(computed_signature);
    
    received_signature == computed_signature_hex
}

// Step 6: Execute the shutdown or sleep command
fn execute_command(command: &str) -> Result<(), std::io::Error> {
    println!("[agent] Executing command: {}", command);
    std::process::Command::new("sh")
        .arg("-c")
        .arg(command)
        .spawn()?
        .wait()?;
    Ok(())
}
