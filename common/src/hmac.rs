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

fn create_hmac_message(command: &str) -> String {
    format!("{}|{}", unix_time_seconds(), command)
}

fn sign_hmac(message: &str, secret: &str) -> String {
    let mac = create_hmac(message, secret.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

pub fn create_signed_message(message: &str, secret: &str) -> String {
    let message = create_hmac_message(message);
    let signature = sign_hmac(&message, &secret);
    format!("{}|{}", message, signature)
}

pub enum HmacValidationResult {
    Valid(String),
    InvalidTimestamp,
    InvalidHmac,
    MalformedMessage,
}

pub fn validate_hmac_message(data: &str, secret: &str) -> HmacValidationResult {
    if let Some((timestamp, message, received_signature)) = parse_hmac_message(data) {
        if !is_timestamp_in_valid_range(timestamp) {
            return HmacValidationResult::InvalidTimestamp;
        }
        if !verify_hmac(&format!("{}|{}", timestamp, message), &received_signature, secret) {
            return HmacValidationResult::InvalidHmac;
        }
        return HmacValidationResult::Valid(message);
    }
    HmacValidationResult::MalformedMessage
}

fn verify_hmac(message: &str, received_signature: &str, secret: &str) -> bool {
    received_signature == sign_hmac(message, secret)
}

fn is_timestamp_in_valid_range(timestamp: u64) -> bool {
    unix_time_seconds().abs_diff(timestamp) <= ALLOWED_WINDOW
}

fn parse_hmac_message(data: &str) -> Option<(u64, String, String)> {
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
