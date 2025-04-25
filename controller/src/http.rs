use std::net::TcpStream;
use std::io::{Write, Read};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use hmac::{Hmac, Mac};
use sha2::Sha256;
use hex;
use tiny_http::{Method, Response, Server};

use crate::config::ControllerConfig;
use crate::wol::send_magic_packet;

pub fn start_http_server(config: ControllerConfig) {
    let server = Server::http("0.0.0.0:8080").expect("Failed to start HTTP server");
    let config = Arc::new(config);
    println!("[controller] HTTP server running on http://0.0.0.0:8080");

    for request in server.incoming_requests() {
        let config = Arc::clone(&config);
        let url = request.url().to_string();
        let method = request.method();

        match (method, url.as_str()) {
            (&Method::Post, path) if path.starts_with("/wake/") => {
                let hostname = path.trim_start_matches("/wake/");
                handle_wake_request(request, hostname, &config);
            }
            (&Method::Post, path) if path.starts_with("/shutdown/") => {
                let hostname = path.trim_start_matches("/shutdown/");
                handle_shutdown_request(request, hostname, &config);
            }
            (&Method::Get, path) if path.starts_with("/status/") => {
                let hostname = path.trim_start_matches("/status/");
                handle_status_request(request, hostname, &config);
            }
            _ => {
                let response = Response::from_string("Not Found").with_status_code(404);
                let _ = request.respond(response);
            }
        }
    }
}

fn handle_status_request(request: tiny_http::Request, hostname: &str, config: &ControllerConfig) {
    let host = match config.hosts.get(hostname) {
        Some(h) => h,
        None => {
            let _ = request.respond(Response::from_string("Unknown host").with_status_code(404));
            return;
        }
    };

    let addr = format!("{}:{}", host.ip, host.port);
    let status = match TcpStream::connect_timeout(
        &addr.parse().unwrap(),
        std::time::Duration::from_secs(2),
    ) {
        Ok(_) => "online",
        Err(_) => "offline",
    };

    let _ = request.respond(Response::from_string(status));
}

fn handle_wake_request(request: tiny_http::Request, hostname: &str, config: &ControllerConfig) {
    if let Some(host) = config.hosts.get(hostname) {
        let result = send_magic_packet(&host.mac, "255.255.255.255");
        let response_text = match result {
            Ok(_) => format!("Magic packet sent to {}", hostname),
            Err(e) => format!("Failed to send packet: {}", e),
        };
        let _ = request.respond(Response::from_string(response_text));
    } else {
        let _ = request.respond(Response::from_string("Unknown host").with_status_code(404));
    }
}

fn handle_shutdown_request(request: tiny_http::Request, hostname: &str, config: &ControllerConfig) {
    let host = match config.hosts.get(hostname) {
        Some(h) => h,
        None => {
            let _ = request.respond(Response::from_string("Unknown host").with_status_code(404));
            return;
        }
    };

    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let message = format!("{}|shutdown", timestamp);
    let signature = sign_hmac(&message, &host.shared_secret);

    let full_message = format!("{}|{}", message, signature);

    match send_shutdown(&host.ip, host.port, &full_message) {
        Ok(response) => {
            let _ = request.respond(Response::from_string(response));
        }
        Err(e) => {
            let _ = request.respond(Response::from_string(format!("Failed: {}", e)));
        }
    }
}

fn sign_hmac(message: &str, secret: &str) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
        .expect("HMAC key setup failed");
    mac.update(message.as_bytes());
    let result = mac.finalize().into_bytes();
    hex::encode(result)
}

fn send_shutdown(ip: &str, port: u16, message: &str) -> Result<String, String> {
    let addr = format!("{}:{}", ip, port);
    let mut stream = TcpStream::connect(addr).map_err(|e| e.to_string())?;
    stream.write_all(message.as_bytes()).map_err(|e| e.to_string())?;

    let mut response = String::new();
    stream.read_to_string(&mut response).map_err(|e| e.to_string())?;
    Ok(response)
}
