//! Tailnet HTTP API dispatch (testable without binding a socket).

use crate::orch_graph::{
    find_orch_graph_from_cwd, find_orch_run, list_live_orch_runs, list_orch_graph_runs,
    load_orch_graph, load_orch_graph_at, OrchGraphError,
};
use crate::orch_status::load_orch_run_status;
use crate::tmux::{capture_pane_text, list_tmux_sessions, send_line_to_session};
use crate::{grouped_for_list_ex, GroupedSessions, ProjectsConfig, SessionsConfig};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use std::path::{Path, PathBuf};

pub const PANE_CAPTURE_LINES: i32 = 200;

#[derive(Debug, Clone, Serialize)]
pub struct TailnetResponse {
    pub status: u16,
    pub body: String,
}

#[derive(Debug, Clone, Serialize)]
struct ErrorEnvelope {
    error: ErrorBody,
}

#[derive(Debug, Clone, Serialize)]
struct ErrorBody {
    code: String,
    message: String,
}

#[derive(Debug, Clone)]
pub enum TailnetApiError {
    NotFound(String),
    BadRequest(String),
    Unauthorized(String),
    Tmux(String),
}

impl TailnetApiError {
    fn code(&self) -> &'static str {
        match self {
            TailnetApiError::NotFound(_) => "not_found",
            TailnetApiError::BadRequest(_) => "bad_request",
            TailnetApiError::Unauthorized(_) => "unauthorized",
            TailnetApiError::Tmux(_) => "tmux_error",
        }
    }

    fn message(&self) -> String {
        match self {
            TailnetApiError::NotFound(m)
            | TailnetApiError::BadRequest(m)
            | TailnetApiError::Unauthorized(m)
            | TailnetApiError::Tmux(m) => m.clone(),
        }
    }
}

/// HTTP status for a backend error (routes with a more specific status map it themselves).
fn status_of(err: &TailnetApiError) -> u16 {
    match err {
        TailnetApiError::NotFound(_) => 404,
        TailnetApiError::BadRequest(_) => 400,
        TailnetApiError::Unauthorized(_) => 401,
        TailnetApiError::Tmux(_) => 502,
    }
}

fn json_error(status: u16, err: TailnetApiError) -> TailnetResponse {
    let body = ErrorEnvelope {
        error: ErrorBody {
            code: err.code().to_string(),
            message: err.message(),
        },
    };
    TailnetResponse {
        status,
        body: serde_json::to_string(&body).unwrap_or_else(|_| {
            r#"{"error":{"code":"internal","message":"serialize"}}"#.to_string()
        }),
    }
}

fn json_ok<T: Serialize>(status: u16, value: &T) -> TailnetResponse {
    TailnetResponse {
        status,
        body: serde_json::to_string(value).unwrap_or_else(|_| "{}".into()),
    }
}

#[derive(Debug, Serialize)]
pub struct HealthPayload {
    pub service: &'static str,
    pub version: String,
    pub tmux_reachable: bool,
}

#[derive(Debug, Serialize)]
pub struct PanePayload {
    pub text: String,
}

#[derive(Debug, Serialize)]
pub struct SendOkPayload {
    pub ok: bool,
}

#[derive(Debug, Deserialize)]
struct SendRequest {
    line: String,
    #[serde(default)]
    enter: bool,
}

/// Parse `CDAMUX_TAILNET_BIND_ADDR`; unspecified addresses (`0.0.0.0`, `::`) are refused (ADR-004).
pub fn parse_tailnet_bind_addr(raw: &str) -> Result<IpAddr, String> {
    let ip: IpAddr = raw
        .trim()
        .parse()
        .map_err(|e| format!("CDAMUX_TAILNET_BIND_ADDR: {e}"))?;
    if ip.is_unspecified() {
        return Err(format!(
            "CDAMUX_TAILNET_BIND_ADDR={raw}: refusing to bind an unspecified address; use the tailnet IP or 127.0.0.1"
        ));
    }
    Ok(ip)
}

/// Resolve bind address for tailnet HTTP (never `0.0.0.0`).
pub fn resolve_tailnet_bind_ip() -> Result<IpAddr, String> {
    if let Ok(raw) = std::env::var("CDAMUX_TAILNET_BIND_ADDR") {
        return parse_tailnet_bind_addr(&raw);
    }
    let output = std::process::Command::new("tailscale")
        .args(["ip", "-4"])
        .output()
        .map_err(|e| format!("tailscale ip -4: {e}"))?;
    if !output.status.success() {
        return Err("tailscale ip -4 failed".into());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout
        .lines()
        .find(|l| !l.trim().is_empty())
        .map(str::trim)
        .unwrap_or("");
    if line.is_empty() {
        return Err("tailscale ip -4 returned no address".into());
    }
    line.parse().map_err(|e| format!("parse tailscale IP: {e}"))
}

/// Header carrying the shared secret (`CDAMUX_TAILNET_TOKEN`).
pub const TAILNET_TOKEN_HEADER: &str = "X-Cdamux-Token";

/// Gate a `/v1/*` request: when `expected` (the env token) is set, `provided` (the header) must
/// match byte-for-byte; when unset, every request passes. Returns the 401 response on mismatch.
pub fn check_tailnet_token(
    expected: Option<&str>,
    provided: Option<&str>,
) -> Result<(), TailnetResponse> {
    let Some(expected) = expected else {
        return Ok(());
    };
    // Constant-time compare without a new dependency: length check then full byte fold.
    let provided = provided.unwrap_or("").as_bytes();
    let expected = expected.as_bytes();
    let mut diff = (provided.len() != expected.len()) as u8;
    for i in 0..expected.len() {
        diff |= expected[i] ^ provided.get(i).copied().unwrap_or(0);
    }
    if diff == 0 {
        Ok(())
    } else {
        Err(json_error(
            401,
            TailnetApiError::Unauthorized(format!("missing or invalid {TAILNET_TOKEN_HEADER}")),
        ))
    }
}

pub trait TailnetApiBackend {
    fn version(&self) -> &str;
    fn tmux_reachable(&self) -> bool;
    fn sessions_grouped(&self) -> GroupedSessions;
    fn pane_tail(&self, session_id: &str) -> Result<String, TailnetApiError>;
    fn send_line(&self, session_id: &str, line: &str, enter: bool) -> Result<(), TailnetApiError>;
    /// Absolute repo path allowed for orch reads (must sit under configured project roots).
    fn resolve_orch_repo(&self, repo: &str) -> Result<PathBuf, TailnetApiError> {
        let _ = repo;
        Err(TailnetApiError::BadRequest(
            "orch repo lookup not supported on this backend".into(),
        ))
    }
}

pub struct LiveTailnetBackend {
    pub version: String,
    projects: ProjectsConfig,
    sessions: SessionsConfig,
    idle_seconds_for_needs_input: u32,
    working_warn_seconds: u32,
}

impl LiveTailnetBackend {
    pub fn new(
        version: String,
        projects: ProjectsConfig,
        sessions: SessionsConfig,
        idle_seconds_for_needs_input: u32,
        working_warn_seconds: u32,
    ) -> Self {
        Self {
            version,
            projects,
            sessions,
            idle_seconds_for_needs_input,
            working_warn_seconds,
        }
    }
}

impl TailnetApiBackend for LiveTailnetBackend {
    fn version(&self) -> &str {
        &self.version
    }

    fn tmux_reachable(&self) -> bool {
        list_tmux_sessions().is_ok()
    }

    fn sessions_grouped(&self) -> GroupedSessions {
        grouped_for_list_ex(
            self.sessions.group_by,
            &self.projects,
            self.idle_seconds_for_needs_input,
            self.working_warn_seconds,
        )
    }

    fn pane_tail(&self, session_id: &str) -> Result<String, TailnetApiError> {
        session_path(session_id)?;
        // Plain text: the dashboard renders with textContent, ANSI escapes would show raw.
        capture_pane_text(session_id, PANE_CAPTURE_LINES, false)
            .map_err(|e| TailnetApiError::Tmux(e.to_string()))
    }

    fn send_line(&self, session_id: &str, line: &str, enter: bool) -> Result<(), TailnetApiError> {
        session_path(session_id)?;
        send_line_to_session(session_id, line, enter)
            .map_err(|e| TailnetApiError::Tmux(e.to_string()))
    }

    fn resolve_orch_repo(&self, repo: &str) -> Result<PathBuf, TailnetApiError> {
        resolve_orch_repo_path(&self.projects, repo)
    }
}

fn resolve_orch_repo_path(projects: &ProjectsConfig, repo: &str) -> Result<PathBuf, TailnetApiError> {
    if repo.is_empty() {
        return Err(TailnetApiError::BadRequest("missing repo query".into()));
    }
    if !Path::new(repo).is_absolute() {
        return Err(TailnetApiError::BadRequest(
            "repo must be an absolute path".into(),
        ));
    }
    // Same 404 for "does not exist" and "outside roots": the body must not act as a path oracle.
    let not_found = || TailnetApiError::NotFound("repo not under configured project roots".into());
    // Canonicalize both sides so `..` and symlinks cannot escape a root.
    let path = std::fs::canonicalize(repo).map_err(|_| not_found())?;
    let under_root = projects
        .roots
        .iter()
        .filter_map(|r| std::fs::canonicalize(&r.path).ok())
        .any(|root| path.starts_with(&root));
    if under_root {
        Ok(path)
    } else {
        Err(not_found())
    }
}

fn query_param(query: Option<&str>, key: &str) -> Option<String> {
    let q = query?;
    for pair in q.split('&') {
        let Some((k, v)) = pair.split_once('=') else {
            continue;
        };
        if k == key {
            return Some(decode_path_segment(v));
        }
    }
    None
}

/// Path of the tmux session named exactly `session_id` (also the existence check).
fn session_path(session_id: &str) -> Result<String, TailnetApiError> {
    list_tmux_sessions()
        .unwrap_or_default()
        .into_iter()
        .find(|s| s.name == session_id)
        .map(|s| s.path)
        .ok_or_else(|| TailnetApiError::NotFound(format!("session not found: {session_id}")))
}

/// Percent-decode a single URL path segment (enough for tmux session names).
pub fn decode_path_segment(segment: &str) -> String {
    let bytes = segment.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = hex(bytes[i + 1]);
            let lo = hex(bytes[i + 2]);
            if let (Some(h), Some(l)) = (hi, lo) {
                out.push((h << 4) | l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

pub fn dispatch_tailnet_request(
    method: &str,
    path: &str,
    body: &str,
    backend: &impl TailnetApiBackend,
) -> TailnetResponse {
    let (path, query) = match path.split_once('?') {
        Some((p, q)) => (p, Some(q)),
        None => (path, None),
    };
    if path == "/v1/health" {
        if method != "GET" {
            return method_not_allowed();
        }
        return json_ok(
            200,
            &HealthPayload {
                service: "cdamuxd",
                version: backend.version().to_string(),
                tmux_reachable: backend.tmux_reachable(),
            },
        );
    }
    if path == "/v1/sessions" {
        if method != "GET" {
            return method_not_allowed();
        }
        return json_ok(200, &backend.sessions_grouped());
    }
    if path == "/v1/orch-graph" {
        if method != "GET" {
            return method_not_allowed();
        }
        let Some(repo) = query_param(query, "repo") else {
            return json_error(
                400,
                TailnetApiError::BadRequest("missing repo query".into()),
            );
        };
        let repo_path = match backend.resolve_orch_repo(&repo) {
            Ok(p) => p,
            Err(e) => return json_error(status_of(&e), e),
        };
        // `run` picks one run (live or archived); default = latest (current.json).
        let loaded = match query_param(query, "run") {
            Some(run) => match find_orch_run(&repo_path, &run) {
                Some(p) => load_orch_graph_at(&p),
                None => {
                    return json_error(404, TailnetApiError::NotFound("no such run under repo".into()))
                }
            },
            None => load_orch_graph(&repo_path),
        };
        return match loaded {
            Ok((graph, _)) => json_ok(200, &graph),
            Err(OrchGraphError::NotFound(_)) => json_error(
                404,
                TailnetApiError::NotFound("no run under $CDAMUX_HOME/runs for repo".into()),
            ),
            Err(OrchGraphError::Invalid { message, .. }) => json_error(
                422,
                TailnetApiError::BadRequest(format!("invalid run graph: {message}")),
            ),
        };
    }
    if path == "/v1/orch-status" {
        if method != "GET" {
            return method_not_allowed();
        }
        let Some(repo) = query_param(query, "repo") else {
            return json_error(
                400,
                TailnetApiError::BadRequest("missing repo query".into()),
            );
        };
        let repo_path = match backend.resolve_orch_repo(&repo) {
            Ok(p) => p,
            Err(e) => return json_error(status_of(&e), e),
        };
        return match load_orch_run_status(&repo_path) {
            Some(st) => json_ok(200, &st),
            None => json_error(
                404,
                TailnetApiError::NotFound("no workers under $CDAMUX_HOME for repo".into()),
            ),
        };
    }
    if path == "/v1/orch-runs" {
        if method != "GET" {
            return method_not_allowed();
        }
        let Some(repo) = query_param(query, "repo") else {
            return json_error(
                400,
                TailnetApiError::BadRequest("missing repo query".into()),
            );
        };
        let repo_path = match backend.resolve_orch_repo(&repo) {
            Ok(p) => p,
            Err(e) => return json_error(status_of(&e), e),
        };
        #[derive(Serialize)]
        struct RunRow {
            run_id: String,
            status: Option<String>,
            path: String,
            session: Option<String>,
            live: bool,
        }
        // Live runs first (newest first), then the archive.
        let rows: Vec<RunRow> = list_live_orch_runs(&repo_path)
            .into_iter()
            .chain(list_orch_graph_runs(&repo_path, 20))
            .map(|r| RunRow {
                run_id: r.run_id,
                status: r.status,
                path: r.path.display().to_string(),
                session: r.session,
                live: r.live,
            })
            .collect();
        return json_ok(200, &rows);
    }
    if let Some(rest) = path.strip_prefix("/v1/sessions/") {
        let parts: Vec<&str> = rest.split('/').collect();
        if parts.len() == 2 {
            let session_id = decode_path_segment(parts[0]);
            let action = parts[1];
            if action == "pane" && method == "GET" {
                match backend.pane_tail(&session_id) {
                    Ok(text) => return json_ok(200, &PanePayload { text }),
                    Err(e) => return json_error(status_of(&e), e),
                }
            }
            if action == "orch-graph" && method == "GET" {
                // One `tmux list-sessions`, not full enrichment: the dashboard polls this at 1 Hz.
                let cwd = match session_path(&session_id) {
                    Ok(p) => p,
                    Err(e) => return json_error(status_of(&e), e),
                };
                match find_orch_graph_from_cwd(&cwd) {
                    Ok((graph, _)) => return json_ok(200, &graph),
                    Err(OrchGraphError::NotFound(_)) => {
                        return json_error(
                            404,
                            TailnetApiError::NotFound(
                                "no $CDAMUX_HOME run for this session cwd".into(),
                            ),
                        );
                    }
                    Err(OrchGraphError::Invalid { path, message }) => {
                        return json_error(
                            422,
                            TailnetApiError::BadRequest(format!(
                                "invalid run graph at {}: {message}",
                                path.display()
                            )),
                        );
                    }
                }
            }
            if action == "send" && method == "POST" {
                let req: SendRequest = match serde_json::from_str(body) {
                    Ok(r) => r,
                    Err(e) => {
                        return json_error(
                            400,
                            TailnetApiError::BadRequest(format!("invalid JSON body: {e}")),
                        );
                    }
                };
                // Typed literally into an agent shell: no newlines (each would submit) or escapes.
                if req.line.chars().any(char::is_control) {
                    return json_error(
                        400,
                        TailnetApiError::BadRequest("line must not contain control characters".into()),
                    );
                }
                match backend.send_line(&session_id, &req.line, req.enter) {
                    Ok(()) => return json_ok(200, &SendOkPayload { ok: true }),
                    Err(e) => return json_error(status_of(&e), e),
                }
            }
        }
    }
    json_error(404, TailnetApiError::NotFound("unknown route".into()))
}

fn method_not_allowed() -> TailnetResponse {
    json_error(405, TailnetApiError::BadRequest("method not allowed".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{EnrichedSession, SessionGroup, SessionState};
    use crate::GroupBy;

    struct MockBackend {
        version: String,
        tmux_ok: bool,
        grouped: GroupedSessions,
        pane: Option<String>,
        send_ok: bool,
        projects: ProjectsConfig,
    }

    fn mock_backend() -> MockBackend {
        MockBackend {
            version: "0".into(),
            tmux_ok: true,
            grouped: mock_grouped(),
            pane: None,
            send_ok: true,
            projects: ProjectsConfig::default(),
        }
    }

    impl TailnetApiBackend for MockBackend {
        fn resolve_orch_repo(&self, repo: &str) -> Result<PathBuf, TailnetApiError> {
            resolve_orch_repo_path(&self.projects, repo)
        }

        fn version(&self) -> &str {
            &self.version
        }

        fn tmux_reachable(&self) -> bool {
            self.tmux_ok
        }

        fn sessions_grouped(&self) -> GroupedSessions {
            self.grouped.clone()
        }

        fn pane_tail(&self, session_id: &str) -> Result<String, TailnetApiError> {
            if session_id == "c-missing" {
                return Err(TailnetApiError::NotFound("session not found".into()));
            }
            Ok(self.pane.clone().unwrap_or_default())
        }

        fn send_line(&self, _session_id: &str, _line: &str, _enter: bool) -> Result<(), TailnetApiError> {
            if self.send_ok {
                Ok(())
            } else {
                Err(TailnetApiError::Tmux("send failed".into()))
            }
        }
    }

    fn mock_grouped() -> GroupedSessions {
        GroupedSessions {
            group_by: GroupBy::Project,
            groups: vec![SessionGroup {
                key: "proj".into(),
                sessions: vec![EnrichedSession {
                    name: "c-foo".into(),
                    path: "/tmp".into(),
                    group_key: "proj".into(),
                    state: SessionState::Idle,
                    agent: "—".into(),
                    issue: None,
                    ..Default::default()
                }],
            }],
        }
    }

    #[test]
    fn health_returns_version_and_tmux_flag() {
        let backend = MockBackend {
            version: "0.0.1".into(),
            ..mock_backend()
        };
        let resp = dispatch_tailnet_request("GET", "/v1/health", "", &backend);
        assert_eq!(resp.status, 200);
        assert!(resp.body.contains("\"tmux_reachable\":true"));
        assert!(resp.body.contains("0.0.1"));
    }

    #[test]
    fn sessions_returns_grouped_json() {
        let backend = mock_backend();
        let resp = dispatch_tailnet_request("GET", "/v1/sessions", "", &backend);
        assert_eq!(resp.status, 200);
        assert!(resp.body.contains("c-foo"));
    }

    #[test]
    fn pane_not_found_is_structured_404() {
        let backend = MockBackend {
            pane: Some("hi".into()),
            ..mock_backend()
        };
        let resp = dispatch_tailnet_request("GET", "/v1/sessions/c-missing/pane", "", &backend);
        assert_eq!(resp.status, 404);
        assert!(resp.body.contains("\"code\":\"not_found\""));
    }

    #[test]
    fn orch_graph_missing_file_is_404() {
        let backend = mock_backend();
        let resp = dispatch_tailnet_request(
            "GET",
            "/v1/sessions/c-foo/orch-graph",
            "",
            &backend,
        );
        assert_eq!(resp.status, 404);
    }

    #[test]
    fn send_accepts_json_body() {
        let backend = mock_backend();
        let resp = dispatch_tailnet_request(
            "POST",
            "/v1/sessions/c-foo/send",
            r#"{"line":"hello","enter":true}"#,
            &backend,
        );
        assert_eq!(resp.status, 200);
        assert!(resp.body.contains("\"ok\":true"));
    }

    #[test]
    fn send_rejects_control_characters_with_400() {
        let backend = mock_backend();
        for line in [r#"a\nb"#, r#"a\rb"#, r#"\u001b[A"#, r#"a\tb"#] {
            let body = format!(r#"{{"line":"{line}","enter":false}}"#);
            let resp = dispatch_tailnet_request("POST", "/v1/sessions/c-foo/send", &body, &backend);
            assert_eq!(resp.status, 400, "{line}: {}", resp.body);
            assert!(resp.body.contains("control characters"), "{}", resp.body);
        }
    }

    #[test]
    fn status_of_maps_error_kinds() {
        assert_eq!(status_of(&TailnetApiError::NotFound(String::new())), 404);
        assert_eq!(status_of(&TailnetApiError::BadRequest(String::new())), 400);
        assert_eq!(status_of(&TailnetApiError::Unauthorized(String::new())), 401);
        assert_eq!(status_of(&TailnetApiError::Tmux(String::new())), 502);
    }

    #[test]
    fn decode_path_segment_percent() {
        assert_eq!(decode_path_segment("c%2Dfoo"), "c-foo");
    }

    #[test]
    fn bind_addr_refuses_unspecified() {
        assert!(parse_tailnet_bind_addr("0.0.0.0").unwrap_err().contains("CDAMUX_TAILNET_BIND_ADDR"));
        assert!(parse_tailnet_bind_addr("::").is_err());
        assert!(parse_tailnet_bind_addr("not-an-ip").is_err());
        assert_eq!(parse_tailnet_bind_addr("100.64.0.7").unwrap().to_string(), "100.64.0.7");
        assert!(parse_tailnet_bind_addr("127.0.0.1").is_ok());
    }

    /// Temp root + CDAMUX_HOME with one running repo graph. Caller must hold `env_lock`
    /// and clear `CDAMUX_HOME` when done.
    fn backend_with_root() -> (tempfile::TempDir, MockBackend) {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        let home = dir.path().join("cdamux");
        std::fs::create_dir_all(&repo).unwrap();
        std::fs::create_dir_all(home.join("runs")).unwrap();
        std::env::set_var("CDAMUX_HOME", &home);
        let body = format!(
            r#"{{"run_id":"r1","repo":"{}","status":"running","nodes":[],"edges":[]}}"#,
            repo.display()
        );
        std::fs::write(home.join("runs/r1.json"), body).unwrap();
        let backend = MockBackend {
            projects: ProjectsConfig {
                roots: vec![crate::ProjectRoot {
                    path: dir.path().to_path_buf(),
                    label: String::new(),
                }],
                ..ProjectsConfig::default()
            },
            ..mock_backend()
        };
        (dir, backend)
    }

    #[test]
    fn orch_graph_repo_query_serves_repo_under_root() {
        let _g = crate::paths::env_lock();
        let (dir, backend) = backend_with_root();
        let url = format!("/v1/orch-graph?repo={}/repo", dir.path().display());
        let resp = dispatch_tailnet_request("GET", &url, "", &backend);
        assert_eq!(resp.status, 200, "{}", resp.body);
        assert!(resp.body.contains("\"run_id\":\"r1\""));
        std::env::remove_var("CDAMUX_HOME");
    }

    #[test]
    fn orch_graph_run_param_picks_a_live_run_and_runs_lists_it() {
        let _g = crate::paths::env_lock();
        let (dir, backend) = backend_with_root();
        let repo = dir.path().join("repo");
        let home = std::env::var("CDAMUX_HOME").unwrap();
        let body = format!(
            r#"{{"run_id":"r2","repo":"{}","session":"alpha","status":"running","nodes":[],"edges":[]}}"#,
            repo.display()
        );
        std::fs::write(format!("{home}/runs/r2.json"), &body).unwrap();
        let f = std::fs::File::open(format!("{home}/runs/r2.json")).unwrap();
        f.set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(5))
            .unwrap();
        let base = format!("/v1/orch-graph?repo={}", repo.display());
        let latest = dispatch_tailnet_request("GET", &base, "", &backend);
        assert!(
            latest.body.contains("\"run_id\":\"r2\""),
            "default is newest live run: {}",
            latest.body
        );
        let r2 = dispatch_tailnet_request("GET", &format!("{base}&run=r2"), "", &backend);
        assert_eq!(r2.status, 200, "{}", r2.body);
        assert!(r2.body.contains("\"session\":\"alpha\""), "{}", r2.body);
        let gone = dispatch_tailnet_request("GET", &format!("{base}&run=..%2Fcurrent"), "", &backend);
        assert_eq!(gone.status, 404, "{}", gone.body);
        let runs =
            dispatch_tailnet_request("GET", &format!("/v1/orch-runs?repo={}", repo.display()), "", &backend);
        assert!(
            runs.body.contains(r#""run_id":"r2","status":"running""#),
            "{}",
            runs.body
        );
        assert!(
            runs.body.contains(r#""session":"alpha","live":true"#),
            "{}",
            runs.body
        );
        std::env::remove_var("CDAMUX_HOME");
    }

    #[test]
    fn orch_graph_repo_traversal_is_404_without_path_echo() {
        let _g = crate::paths::env_lock();
        let (dir, backend) = backend_with_root();
        let escaped = format!("{}/repo/../../../../../../etc", dir.path().display());
        let resp = dispatch_tailnet_request(
            "GET",
            &format!("/v1/orch-graph?repo={escaped}"),
            "",
            &backend,
        );
        assert_eq!(resp.status, 404, "{}", resp.body);
        assert!(!resp.body.contains("etc"), "body must not echo the path: {}", resp.body);

        let resp2 = dispatch_tailnet_request("GET", "/v1/orch-graph?repo=/nonexistent/zzz", "", &backend);
        assert_eq!(resp2.status, 404);
        assert_eq!(resp.body, resp2.body);
        std::env::remove_var("CDAMUX_HOME");
    }

    #[test]
    fn orch_graph_repo_must_be_absolute() {
        let _g = crate::paths::env_lock();
        let (_dir, backend) = backend_with_root();
        let resp = dispatch_tailnet_request("GET", "/v1/orch-graph?repo=relative/x", "", &backend);
        assert_eq!(resp.status, 400);
        std::env::remove_var("CDAMUX_HOME");
    }

    #[test]
    fn token_check_rejects_missing_or_wrong_header() {
        let missing = check_tailnet_token(Some("s3cret"), None).unwrap_err();
        assert_eq!(missing.status, 401);
        assert!(missing.body.contains("\"code\":\"unauthorized\""));
        assert_eq!(check_tailnet_token(Some("s3cret"), Some("s3cre")).unwrap_err().status, 401);
        assert_eq!(check_tailnet_token(Some("s3cret"), Some("s3cretX")).unwrap_err().status, 401);
        assert_eq!(check_tailnet_token(Some("s3cret"), Some("S3CRET")).unwrap_err().status, 401);
    }

    #[test]
    fn token_check_passes_when_matching_or_unset() {
        assert!(check_tailnet_token(Some("s3cret"), Some("s3cret")).is_ok());
        assert!(check_tailnet_token(None, None).is_ok());
        assert!(check_tailnet_token(None, Some("anything")).is_ok());
    }
}
