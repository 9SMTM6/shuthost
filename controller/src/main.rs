mod wol;
mod config;
mod http;

fn main() {
    if let Err(e) = wol::send_magic_packet("AA:BB:CC:DD:EE:FF", "192.168.1.255") {
        eprintln!("Failed to send WoL packet: {}", e);
    }
}
