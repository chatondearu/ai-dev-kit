//! Optional HTTP API on the Tailscale interface (`daemon.listen_tailnet`).

use cdamux_core::{
    dispatch_tailnet_request, resolve_tailnet_bind_ip, LiveTailnetBackend, ProjectsConfig,
    SessionsConfig,
};
use std::net::SocketAddr;
use std::sync::Arc;
use std::thread;
use tiny_http::{Header, Method, Response, Server, StatusCode};
use tracing::{info, warn};

pub fn spawn_tailnet_server(
    port: u16,
    version: String,
    projects: ProjectsConfig,
    sessions: SessionsConfig,
    idle_seconds_for_needs_input: u32,
) {
    thread::spawn(move || {
        if let Err(e) = run_tailnet_server(
            port,
            version,
            projects,
            sessions,
            idle_seconds_for_needs_input,
        ) {
            warn!("tailnet HTTP server stopped: {e}");
        }
    });
}

fn run_tailnet_server(
    port: u16,
    version: String,
    projects: ProjectsConfig,
    sessions: SessionsConfig,
    idle_seconds_for_needs_input: u32,
) -> Result<(), String> {
    let ip = resolve_tailnet_bind_ip()?;
    let addr = SocketAddr::from((ip, port));
    let server = Server::http(addr).map_err(|e| format!("bind {addr}: {e}"))?;
    info!(%addr, "tailnet HTTP listening");

    let backend = Arc::new(LiveTailnetBackend::new(
        version,
        projects,
        sessions,
        idle_seconds_for_needs_input,
    ));

    for request in server.incoming_requests() {
        let backend = Arc::clone(&backend);
        thread::spawn(move || handle_one(request, backend));
    }
    Ok(())
}

fn handle_one(mut request: tiny_http::Request, backend: Arc<LiveTailnetBackend>) {
    let method = match request.method() {
        Method::Get => "GET",
        Method::Post => "POST",
        _ => {
            let _ = request.respond(Response::from_string("").with_status_code(405));
            return;
        }
    };
    let path = request.url().to_string();
    let mut body = String::new();
    let _ = std::io::Read::read_to_string(request.as_reader(), &mut body);

    let resp = dispatch_tailnet_request(method, &path, &body, backend.as_ref());
    let status = StatusCode(resp.status);
    let mut response = Response::from_string(resp.body).with_status_code(status);
    response = response.with_header(
        Header::from_bytes(&b"Content-Type"[..], &b"application/json; charset=utf-8"[..])
            .expect("content-type header"),
    );
    let _ = request.respond(response);
}
