use crate::config::{ControllerConfig, Host};
use crate::wol::send_magic_packet;
use tiny_http::{Server, Response, Method};
use std::sync::Arc;

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
            _ => {
                let response = Response::from_string("Not Found").with_status_code(404);
                let _ = request.respond(response);
            }
        }
    }
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
