use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::time::{SystemTime, UNIX_EPOCH};

pub const ALLOWED_WINDOW: u64 = 30; // Seconds

fn create_hmac(message: &str, secret: &[u8]) -> Hmac<Sha256> {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret)
        .expect("HMAC can take a key of any size");
    mac.update(message.as_bytes());
    mac
}

pub fn create_hmac_message(command: &str) -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    format!("{}|{}", timestamp, command)
}

pub fn sign_hmac(message: &str, secret: &str) -> String {
    let mac = create_hmac(message, secret.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

pub fn verify_hmac(message: &str, received_signature: &str, secret: &[u8]) -> bool {
    let mac = create_hmac(message, secret);
    let computed_signature = mac.finalize().into_bytes();
    let computed_signature_hex = hex::encode(computed_signature);

    received_signature == computed_signature_hex
}

pub fn is_timestamp_valid(timestamp: u64) -> bool {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    now.abs_diff(timestamp) <= ALLOWED_WINDOW
}

pub fn parse_hmac_message(data: &str) -> Option<(u64, String, String)> {
    let parts: Vec<&str> = data.split('|').collect();
    if parts.len() != 3 {
        return None;
    }

    let timestamp = parts[0].parse().ok()?;
    Some((timestamp, parts[1].to_string(), parts[2].to_string()))
}