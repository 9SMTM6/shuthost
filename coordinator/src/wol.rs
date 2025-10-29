use std::net::UdpSocket;

use eyre::Context;

/// # Errors
///
/// Returns an error if the MAC address is invalid or if the UDP socket cannot be bound or sent.
pub fn send_magic_packet(mac_address: &str, broadcast_ip: &str) -> eyre::Result<()> {
    let mac_bytes = parse_mac(mac_address)?;
    let mut packet = [0xFFu8; 102];

    for i in 0..16 {
        #[expect(
            clippy::indexing_slicing,
            reason = "Should be fine with the provided numbers"
        )]
        packet[(i + 1) * 6..(i + 2) * 6].copy_from_slice(&mac_bytes);
    }

    let socket = UdpSocket::bind("0.0.0.0:0")
        .wrap_err("Failed to bind UDP socket")?;
    socket.set_broadcast(true)
        .wrap_err("Failed to set broadcast on socket")?;

    socket
        .send_to(&packet, format!("{broadcast_ip}:9"))
        .wrap_err(format!("Failed to send magic packet to {broadcast_ip}:9"))?;

    Ok(())
}

fn parse_mac(mac: &str) -> eyre::Result<[u8; 6]> {
    let mut mac_bytes = [0u8; 6];
    let mut parts = mac.split(':');

    for mac_byte in &mut mac_bytes {
        let part = parts.next().ok_or_else(|| eyre::eyre!("Invalid MAC address format: not enough parts"))?;
        *mac_byte = u8::from_str_radix(part, 16).map_err(|_| eyre::eyre!("Invalid MAC byte: {}", part))?;
    }

    // Ensure there are no extra parts
    if parts.next().is_some() {
        return Err(eyre::eyre!("Invalid MAC address format: too many parts"));
    }

    Ok(mac_bytes)
}

/// # Errors
///
/// Returns an error if the socket cannot be bound or configured.
pub fn test_wol_reachability(target_port: u16) -> eyre::Result<bool> {
    let socket = UdpSocket::bind("0.0.0.0:0")
        .wrap_err("Failed to bind socket")?;
    socket
        .set_read_timeout(Some(std::time::Duration::from_secs(1)))
        .wrap_err("Failed to set timeout")?;

    // Test broadcast
    socket
        .set_broadcast(true)
        .wrap_err("Failed to set broadcast")?;

    let test_message = b"SHUTHOST_WOL_TEST_BROADCAST";
    socket
        .send_to(test_message, format!("255.255.255.255:{target_port}"))
        .wrap_err(format!("Failed to send broadcast test to 255.255.255.255:{target_port}"))?;

    let mut buf = [0u8; 32];
    let broadcast_works = socket.recv(&mut buf).is_ok();

    Ok(broadcast_works)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_mac_valid() {
        let mac_str = "01:23:45:67:89:ab";
        let bytes = parse_mac(mac_str).expect("Should parse valid MAC");
        assert_eq!(bytes, [0x01, 0x23, 0x45, 0x67, 0x89, 0xab]);
    }

    #[test]
    fn test_parse_mac_invalid_format() {
        let mac_str = "01:23:45:67:89";
        let err = parse_mac(mac_str).unwrap_err();
        assert!(err.to_string().contains("not enough parts"));
    }

    #[test]
    fn test_parse_mac_invalid_byte() {
        let mac_str = "01:23:45:67:89:zz";
        let err = parse_mac(mac_str).unwrap_err();
        assert!(err.to_string().contains("Invalid MAC byte"));
    }
}
