use std::net::UdpSocket;

pub fn send_magic_packet(mac_address: &str, broadcast_ip: &str) -> Result<(), String> {
    let mac_bytes = parse_mac(mac_address)?;
    let mut packet = [0xFFu8; 102];

    for i in 0..16 {
        packet[(i + 1) * 6..(i + 2) * 6].copy_from_slice(&mac_bytes);
    }

    let socket = UdpSocket::bind("0.0.0.0:0").map_err(|e| e.to_string())?;
    socket.set_broadcast(true).map_err(|e| e.to_string())?;

    socket
        .send_to(&packet, format!("{}:9", broadcast_ip))
        .map_err(|e| e.to_string())?;

    Ok(())
}

fn parse_mac(mac: &str) -> Result<[u8; 6], String> {
    let parts: Vec<&str> = mac.split(':').collect();
    if parts.len() != 6 {
        return Err("Invalid MAC address format".into());
    }

    let mut mac_bytes = [0u8; 6];
    for (i, part) in parts.iter().enumerate() {
        mac_bytes[i] = u8::from_str_radix(part, 16).map_err(|_| "Invalid MAC byte")?;
    }

    Ok(mac_bytes)
}

pub fn test_wol_reachability(target_port: u16) -> Result<bool, String> {
    let socket =
        UdpSocket::bind("0.0.0.0:0").map_err(|e| format!("Failed to bind socket: {}", e))?;
    socket
        .set_read_timeout(Some(std::time::Duration::from_secs(1)))
        .map_err(|e| format!("Failed to set timeout: {}", e))?;

    // Test broadcast
    socket
        .set_broadcast(true)
        .map_err(|e| format!("Failed to set broadcast: {}", e))?;

    let test_message = b"SHUTHOST_WOL_TEST_BROADCAST";
    socket
        .send_to(test_message, format!("255.255.255.255:{}", target_port))
        .map_err(|e| format!("Failed to send broadcast test: {}", e))?;

    let mut buf = [0u8; 32];
    let broadcast_works = socket.recv(&mut buf).is_ok();

    Ok(broadcast_works)
}
