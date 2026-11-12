//! Optional HTTP API on the Tailscale interface (`daemon.listen_tailnet`).

use crate::web_static::lookup_web_asset;
use cdamux_core::{
    check_tailnet_token, dispatch_tailnet_request, resolve_tailnet_bind_ip, LiveTailnetBackend,
    ProjectsConfig, SessionsConfig, TailnetResponse, TAILNET_TOKEN_HEADER,
};
use std::io::Read;
use std::net::SocketAddr;
use std::sync::Arc;
use std::thread;
use tiny_http::{Header, Method, Response, Server, StatusCode};
use tracing::{info, warn};

/// Request bodies above this are refused with 413; the API only takes small JSON.
const MAX_BODY: usize = 64 * 1024;

/// Blocks serving requests; returns `Err` when the bind address cannot be resolved or bound.
pub fn run_tailnet_server(
    port: u16,
    version: String,
    projects: ProjectsConfig,
    sessions: SessionsConfig,
    idle_seconds_for_needs_input: u32,
    working_warn_seconds: u32,
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

    let mut allowed_hosts = vec![addr.to_string()];
    match tailnet_dns_name() {
        Some(name) => allowed_hosts.push(format!("{name}:{port}")),
        None => warn!("no MagicDNS name from `tailscale status --json`: Host must be {addr}"),
    }

    let backend = LiveTailnetBackend::new(
        version,
        projects,
        sessions,
        idle_seconds_for_needs_input,
        working_warn_seconds,
    );
    serve(server, backend, token, allowed_hosts);
    Ok(())
}

/// This machine's MagicDNS name (`.Self.DNSName`, trailing dot stripped), read once at startup.
fn tailnet_dns_name() -> Option<String> {
    let out = std::process::Command::new("tailscale").args(["status", "--json"]).output().ok()?;
    let status: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    let name = status["Self"]["DNSName"].as_str()?.trim_end_matches('.');
    (!name.is_empty()).then(|| name.to_string())
}

/// Accept loop; split from `run_tailnet_server` so tests can bind `127.0.0.1:0`.
fn serve(server: Server, backend: LiveTailnetBackend, token: Option<String>, allowed_hosts: Vec<String>) {
    let backend = Arc::new(backend);
    let ctx = Arc::new((token, allowed_hosts));
    for request in server.incoming_requests() {
        let backend = Arc::clone(&backend);
        let ctx = Arc::clone(&ctx);
        thread::spawn(move || handle_one(request, backend, ctx.0.as_deref(), &ctx.1));
    }
}

fn header<'a>(request: &'a tiny_http::Request, name: &'static str) -> Option<&'a str> {
    request
        .headers()
        .iter()
        .find(|h| h.field.equiv(name))
        .map(|h| h.value.as_str())
}

/// Same envelope as `dispatch_tailnet_request` errors; callers pass fixed messages (no JSON escaping needed).
fn api_error(status: u16, message: &str) -> TailnetResponse {
    TailnetResponse {
        status,
        body: format!(r#"{{"error":{{"code":"bad_request","message":"{message}"}}}}"#),
    }
}

/// Header gate run before any body byte is read. `Host` must be one of `allowed_hosts` (bind
/// `ip:port`, MagicDNS `name:port`): a page on another name resolving here, i.e. DNS rebinding,
/// is refused. POST must be JSON: a foreign page can only send `text/plain` without a CORS
/// preflight, and OPTIONS is 405.
fn check_request_headers(
    allowed_hosts: &[String],
    host: Option<&str>,
    method: &str,
    content_type: Option<&str>,
    body_length: Option<usize>,
) -> Result<(), TailnetResponse> {
    if !host.is_some_and(|h| allowed_hosts.iter().any(|a| a.eq_ignore_ascii_case(h))) {
        return Err(api_error(421, &format!("Host header must be {}", allowed_hosts.join(" or "))));
    }
    let mime = content_type.and_then(|c| c.split(';').next()).map(str::trim);
    if method == "POST" && !mime.is_some_and(|m| m.eq_ignore_ascii_case("application/json")) {
        return Err(api_error(415, "POST requires Content-Type: application/json"));
    }
    if body_length.is_some_and(|n| n > MAX_BODY) {
        return Err(api_error(413, "request body too large"));
    }
    Ok(())
}

fn handle_one(
    mut request: tiny_http::Request,
    backend: Arc<LiveTailnetBackend>,
    token: Option<&str>,
    allowed_hosts: &[String],
) {
    let method = match request.method() {
        Method::Get => "GET",
        Method::Post => "POST",
        _ => {
            let _ = request.respond(Response::from_string("").with_status_code(405));
            return;
        }
    };
    let path = request.url().to_string();

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

    let gate = check_tailnet_token(token, header(&request, TAILNET_TOKEN_HEADER)).and_then(|()| {
        check_request_headers(
            allowed_hosts,
            header(&request, "Host"),
            method,
            header(&request, "Content-Type"),
            request.body_length(),
        )
    });
    let resp = match gate {
        Ok(()) => {
            // `take` bounds chunked bodies too (no Content-Length to check above).
            let mut body = String::new();
            let read = request.as_reader().take(MAX_BODY as u64 + 1).read_to_string(&mut body);
            if body.len() > MAX_BODY {
                api_error(413, "request body too large")
            } else if read.is_err() {
                api_error(400, "request body must be UTF-8")
            } else {
                dispatch_tailnet_request(method, &path, &body, backend.as_ref())
            }
        }
        Err(rejected) => rejected,
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
            600,
        );
        let token = token.map(str::to_string);
        thread::spawn(move || serve(server, backend, token, vec![addr.to_string()]));
        addr
    }

    /// Minimal HTTP/1.0 client: returns (status, body).
    fn get(addr: SocketAddr, path: &str, header: Option<&str>) -> (u16, String) {
        let mut stream = TcpStream::connect(addr).unwrap();
        let extra = header.map(|h| format!("{TAILNET_TOKEN_HEADER}: {h}\r\n")).unwrap_or_default();
        write!(stream, "GET {path} HTTP/1.0\r\nHost: {addr}\r\n{extra}\r\n").unwrap();
        let mut raw = String::new();
        stream.read_to_string(&mut raw).unwrap();
        let status = raw.split_whitespace().nth(1).unwrap().parse().unwrap();
        let body = raw.split_once("\r\n\r\n").map(|(_, b)| b.to_string()).unwrap_or_default();
        (status, body)
    }

    /// Send raw request head; return the status as soon as the response head arrives
    /// (the declared body is never sent, so reading to EOF would wait on the server's drain).
    fn status_of_raw(addr: SocketAddr, head: &str) -> u16 {
        let mut stream = TcpStream::connect(addr).unwrap();
        stream.write_all(head.as_bytes()).unwrap();
        let mut raw = Vec::new();
        let mut buf = [0u8; 512];
        while !raw.windows(4).any(|w| w == b"\r\n\r\n") {
            let n = stream.read(&mut buf).unwrap();
            assert!(n > 0, "connection closed before response head");
            raw.extend_from_slice(&buf[..n]);
        }
        String::from_utf8_lossy(&raw).split_whitespace().nth(1).unwrap().parse().unwrap()
    }

    #[test]
    fn header_gate_rejects_host_content_type_and_size() {
        let bind = "127.0.0.1:7480";
        let allowed = [bind.to_string()];
        let json = Some("application/json; charset=utf-8");
        let check = |host, method, ct, len| check_request_headers(&allowed, host, method, ct, len);
        assert_eq!(check(None, "GET", None, None).unwrap_err().status, 421);
        assert_eq!(check(Some("evil.example:7480"), "GET", None, None).unwrap_err().status, 421);
        assert_eq!(check(Some(bind), "POST", Some("text/plain"), Some(2)).unwrap_err().status, 415);
        assert_eq!(check(Some(bind), "POST", None, Some(2)).unwrap_err().status, 415);
        assert_eq!(check(Some(bind), "POST", json, Some(MAX_BODY + 1)).unwrap_err().status, 413);
        assert!(check(Some(bind), "POST", json, Some(MAX_BODY)).is_ok());
        assert!(check(Some(bind), "GET", None, None).is_ok());
    }

    #[test]
    fn header_gate_accepts_magicdns_host_case_insensitive() {
        let allowed = ["100.64.0.7:7480".to_string(), "box.tail1234.ts.net:7480".to_string()];
        let check = |host| check_request_headers(&allowed, Some(host), "GET", None, None);
        assert!(check("100.64.0.7:7480").is_ok());
        assert!(check("Box.Tail1234.ts.net:7480").is_ok());
        assert_eq!(check("box.tail1234.ts.net:80").unwrap_err().status, 421);
        assert_eq!(check("evil.example:7480").unwrap_err().status, 421);
    }

    #[test]
    fn huge_post_is_rejected_before_its_body_is_read() {
        let head = |addr: SocketAddr, ct: &str| {
            format!(
                "POST /v1/sessions/x/send HTTP/1.1\r\nHost: {addr}\r\nContent-Type: {ct}\r\nContent-Length: 300000000\r\n\r\n"
            )
        };
        let locked = start(Some("s3cret"));
        assert_eq!(status_of_raw(locked, &head(locked, "application/json")), 401);
        let open = start(None);
        assert_eq!(status_of_raw(open, &head(open, "application/json")), 413);
        assert_eq!(status_of_raw(open, &head(open, "text/plain")), 415);
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
