use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

use crate::config::Config;
use crate::handler::handle_request;

pub fn start_agent(config: Config) {
    let addr = format!("0.0.0.0:{}", config.agent.port);
    let listener = TcpListener::bind(&addr).expect("Failed to bind port");
    println!("[agent] Listening on {}", addr);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let config = config.clone();
                thread::spawn(move || {
                    handle_client(stream, config);
                });
            }
            Err(e) => {
                eprintln!("[agent] Connection failed: {}", e);
            }
        }
    }
}

fn handle_client(mut stream: TcpStream, config: Config) {
    let mut buffer = [0u8; 1024];
    match stream.read(&mut buffer) {
        Ok(size) => {
            let data = &buffer[..size];
            let response = handle_request(data, &config);
            let _ = stream.write_all(response.as_bytes());
        }
        Err(e) => {
            eprintln!("[agent] Failed to read from stream: {}", e);
        }
    }
}
