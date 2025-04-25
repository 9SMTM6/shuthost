use std::{env, fs};
use std::net::TcpStream;
use std::io::{Write, Read};
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use hmac::{Hmac, Mac};
use regex::Regex;
use serde_json::json;
use sha2::Sha256;
use hex;
use tiny_http::{Method, Response, Server};

use crate::config::{ControllerConfig, Host};
use crate::wol::send_magic_packet;

pub fn start_http_server(config: ControllerConfig) {
    let server = Server::http("0.0.0.0:8081").expect("Failed to start HTTP server");
    let config = Arc::new(config);
    println!("HTTP server running on http://0.0.0.0:8081");

    let re = Regex::new(r"^/api/(?:wake|shutdown|status)/([^/]+)").unwrap();  // Regex to capture hostname    

    // Get agent binary path from env or fallback to default
    let agent_path_raw = env::var("AGENT_PATH")
        .unwrap_or_else(|_| "target/x86_64-unknown-linux-gnu/release/shuthost_agent".to_string());

    let agent_path = fs::canonicalize(&agent_path_raw)
        .unwrap_or_else(|_| panic!("Agent binary not found at: {}", agent_path_raw));
    println!("Resolved agent binary path: {}", agent_path.display());

    let agent_binary = fs::read(&agent_path)
        .unwrap_or_else(|_| panic!("Failed to read agent binary at: {}", agent_path.display()));

    for request in server.incoming_requests() {
        let config = Arc::clone(&config);
        let url = request.url().to_string();
        let method = request.method();

        let hostaware_endpoint = re.captures(&url);

        let was_hostaware_endpoint = hostaware_endpoint.is_some();

        let hostname = hostaware_endpoint.and_then(|caps| caps.get(1).map(|m| m.as_str()));

        let host = hostname.and_then(|it| config.hosts.get(it));

        if was_hostaware_endpoint && host.is_none() {
            let _ = request.respond(Response::from_string("Unknown host").with_status_code(404));
            continue;
        }

        match (method, url.as_str()) {
            // List hosts: GET /api/hosts
            (&Method::Get, "/api/hosts") => handle_list_hosts(request, &config),

            // Wake endpoint: POST /api/wake/{hostname}
            (&Method::Post, path) if path.starts_with("/api/wake/") => {
                handle_wake_request(request, host.unwrap(), hostname.unwrap());
            }

            // Shutdown endpoint: POST /api/shutdown/{hostname}
            (&Method::Post, path) if path.starts_with("/api/shutdown/") => {
                handle_shutdown_request(request, host.unwrap());
            }

            // Status endpoint: GET /api/status/{hostname}
            (&Method::Get, path) if path.starts_with("/api/status/") => {
                handle_status_request(request, host.unwrap());
            }

            // Allow downloading of the agent
            (&Method::Get, "/download_agent") => handle_download_agent(request, agent_binary.as_slice()),

            // Serve the UI at the root path: GET /
            (&Method::Get, "/") => handle_ui(request),

            _ => {
                let response = Response::from_string("Not Found").with_status_code(404);
                let _ = request.respond(response);
            }
        }
    }
}

// Serve the UI's main page (could be static HTML, or rendered dynamically)
fn handle_ui(request: tiny_http::Request) {
    let html = include_str!("../index.html");
    
    let response = Response::from_string(html)
        .with_header(tiny_http::Header::from_str("Content-Type: text/html").unwrap());
    
    let _ = request.respond(response);
}

fn handle_status_request(request: tiny_http::Request, host: &Host) {
    let addr = format!("{}:{}", host.ip, host.port);
    let status = match TcpStream::connect_timeout(
        &addr.parse().unwrap(),
        std::time::Duration::from_millis(200),
    ) {
        Ok(_) => "online",
        Err(_) => "offline",
    };

    let _ = request.respond(Response::from_string(status));
}

fn handle_wake_request(request: tiny_http::Request, host: &Host, hostname: &str) {
    let result = send_magic_packet(&host.mac, "255.255.255.255");
    let response_text = match result {
        Ok(_) => format!("Magic packet sent to {}", hostname),
        Err(e) => format!("Failed to send packet: {}", e),
    };
    let _ = request.respond(Response::from_string(response_text));
}

fn handle_list_hosts(request: tiny_http::Request, config: &ControllerConfig) {
    let public_hosts: Vec<_> = config.hosts.iter().map(|(name, host)| {
        json!({
            "name": name,
            "ip": host.ip,
            "mac": host.mac,
            "port": host.port
        })
    }).collect();

    let body = serde_json::to_string(&public_hosts).unwrap();
    let response = Response::from_string(body)
        .with_header(tiny_http::Header::from_str("Content-Type: application/json").unwrap());

    let _ = request.respond(response);
}

fn handle_download_agent(request: tiny_http::Request, agent_binary: &[u8]) {
    let response = tiny_http::Response::from_data(
        agent_binary,
    ).with_header(
        tiny_http::Header::from_bytes("Content-Length", agent_binary.len().to_string()).unwrap()
    ).with_header(
        tiny_http::Header::from_str("Content-Type: application/octet-stream").unwrap()
    ).with_status_code(200);
    let _ = request.respond(response);
}


fn handle_shutdown_request(request: tiny_http::Request, host: &Host) {
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
    stream.set_read_timeout(Some(Duration::from_millis(200))).map_err(|err| err.to_string())?;

    let mut response = String::new();
    stream.read_to_string(&mut response).map_err(|e| e.to_string())?;
    Ok(response)
}
