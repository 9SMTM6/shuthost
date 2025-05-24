use hmac::{Hmac, Mac};
use rand::Rng;
use sha2::Sha256;
use std::time::{SystemTime, UNIX_EPOCH};

pub const ALLOWED_WINDOW: u64 = 30; // Seconds

fn create_hmac(message: &str, secret: &[u8]) -> Hmac<Sha256> {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret).expect("HMAC can take a key of any size");
    mac.update(message.as_bytes());
    mac
}

fn unix_time_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs()
}

pub fn create_hmac_message(command: &str) -> String {
    format!("{}|{}", unix_time_seconds(), command)
}

pub fn sign_hmac(message: &str, secret: &str) -> String {
    let mac = create_hmac(message, secret.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

pub fn verify_hmac(message: &str, received_signature: &str, secret: &str) -> bool {
    received_signature == sign_hmac(message, secret)
}

pub fn is_timestamp_in_valid_range(timestamp: u64) -> bool {
    unix_time_seconds().abs_diff(timestamp) <= ALLOWED_WINDOW
}

pub fn parse_hmac_message(data: &str) -> Option<(u64, String, String)> {
    let parts: Vec<&str> = data.split('|').collect();
    if parts.len() != 3 {
        return None;
    }

    let timestamp = parts[0].parse().ok()?;
    Some((timestamp, parts[1].to_string(), parts[2].to_string()))
}

pub fn generate_secret() -> String {
    // Simple random secret generation: 32 characters
    let mut rng = rand::rng();
    (0..32)
        .map(|_| rng.sample(rand::distr::Alphanumeric) as char)
        .collect()
}
