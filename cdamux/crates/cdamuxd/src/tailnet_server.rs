//! Optional HTTP API on the Tailscale interface (`daemon.listen_tailnet`).

use crate::web_static::lookup_web_asset;
use cdamux_core::{
    check_tailnet_token, dispatch_tailnet_request, resolve_tailnet_bind_ip, LiveTailnetBackend,
    ProjectsConfig, SessionsConfig, TAILNET_TOKEN_HEADER,
};
use std::io::Read;
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

    let token = std::env::var("CDAMUX_TAILNET_TOKEN")
        .ok()
        .filter(|t| !t.is_empty());
    if token.is_none() {
        warn!("tailnet API without token: set CDAMUX_TAILNET_TOKEN to require {TAILNET_TOKEN_HEADER}");
    }

    let backend = LiveTailnetBackend::new(version, projects, sessions, idle_seconds_for_needs_input);
    serve(server, backend, token);
    Ok(())
}

/// Accept loop; split from `run_tailnet_server` so tests can bind `127.0.0.1:0`.
fn serve(server: Server, backend: LiveTailnetBackend, token: Option<String>) {
    let backend = Arc::new(backend);
    let token = Arc::new(token);
    for request in server.incoming_requests() {
        let backend = Arc::clone(&backend);
        let token = Arc::clone(&token);
        thread::spawn(move || handle_one(request, backend, token.as_deref()));
    }
}

fn handle_one(mut request: tiny_http::Request, backend: Arc<LiveTailnetBackend>, token: Option<&str>) {
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
    let _ = Read::read_to_string(request.as_reader(), &mut body);

    // Static dashboard assets stay public: the page must load before it can send the header.
    if method == "GET" {
        if let Some((bytes, mime)) = lookup_web_asset(&path) {
            let mut response = Response::from_data(bytes).with_status_code(200);
            response = response.with_header(
                Header::from_bytes(&b"Content-Type"[..], mime.as_bytes()).expect("content-type"),
            );
            let _ = request.respond(response);
            return;
        }
    }

    let provided = request
        .headers()
        .iter()
        .find(|h| h.field.equiv(TAILNET_TOKEN_HEADER))
        .map(|h| h.value.as_str().to_string());
    let resp = match check_tailnet_token(token, provided.as_deref()) {
        Ok(()) => dispatch_tailnet_request(method, &path, &body, backend.as_ref()),
        Err(unauthorized) => unauthorized,
    };
    let status = StatusCode(resp.status);
    let mut response = Response::from_string(resp.body).with_status_code(status);
    response = response.with_header(
        Header::from_bytes(&b"Content-Type"[..], &b"application/json; charset=utf-8"[..])
            .expect("content-type header"),
    );
    let _ = request.respond(response);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::TcpStream;

    fn start(token: Option<&str>) -> SocketAddr {
        let server = Server::http("127.0.0.1:0").unwrap();
        let addr = server.server_addr().to_ip().unwrap();
        let backend = LiveTailnetBackend::new(
            "test".into(),
            ProjectsConfig::default(),
            SessionsConfig::default(),
            60,
        );
        let token = token.map(str::to_string);
        thread::spawn(move || serve(server, backend, token));
        addr
    }

    /// Minimal HTTP/1.0 client: returns (status, body).
    fn get(addr: SocketAddr, path: &str, header: Option<&str>) -> (u16, String) {
        let mut stream = TcpStream::connect(addr).unwrap();
        let extra = header.map(|h| format!("{TAILNET_TOKEN_HEADER}: {h}\r\n")).unwrap_or_default();
        write!(stream, "GET {path} HTTP/1.0\r\nHost: x\r\n{extra}\r\n").unwrap();
        let mut raw = String::new();
        stream.read_to_string(&mut raw).unwrap();
        let status = raw.split_whitespace().nth(1).unwrap().parse().unwrap();
        let body = raw.split_once("\r\n\r\n").map(|(_, b)| b.to_string()).unwrap_or_default();
        (status, body)
    }

    #[test]
    fn token_set_rejects_missing_and_wrong_header_with_json_401() {
        let addr = start(Some("s3cret"));
        let (status, body) = get(addr, "/v1/health", None);
        assert_eq!(status, 401);
        assert!(body.contains("\"code\":\"unauthorized\""), "{body}");
        let (status, _) = get(addr, "/v1/health", Some("wrong"));
        assert_eq!(status, 401);
        let (status, body) = get(addr, "/v1/health", Some("s3cret"));
        assert_eq!(status, 200);
        assert!(body.contains("\"service\":\"cdamuxd\""), "{body}");
    }

    #[test]
    fn token_unset_keeps_api_open() {
        let addr = start(None);
        let (status, _) = get(addr, "/v1/health", None);
        assert_eq!(status, 200);
    }

    #[test]
    fn static_assets_stay_public_with_token_set() {
        let addr = start(Some("s3cret"));
        let (status, _) = get(addr, "/", None);
        assert_eq!(status, 200);
    }
}
