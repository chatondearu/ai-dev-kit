//! Tailnet HTTP API dispatch (testable without binding a socket).

use crate::orch_graph::{
    find_orch_graph_from_cwd, list_orch_graph_runs, load_orch_graph, OrchGraphError,
};
use crate::orch_status::load_orch_run_status;
use crate::project_tree::project_for_path;
use crate::tmux::{capture_pane_text, list_tmux_sessions, send_line_to_session};
use crate::{
    discover_projects, grouped_for_list, GroupedSessions, ProjectsConfig, SessionsConfig,
};
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
    Tmux(String),
}

impl TailnetApiError {
    fn code(&self) -> &'static str {
        match self {
            TailnetApiError::NotFound(_) => "not_found",
            TailnetApiError::BadRequest(_) => "bad_request",
            TailnetApiError::Tmux(_) => "tmux_error",
        }
    }

    fn message(&self) -> String {
        match self {
            TailnetApiError::NotFound(m) => m.clone(),
            TailnetApiError::BadRequest(m) => m.clone(),
            TailnetApiError::Tmux(m) => m.clone(),
        }
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

/// Resolve bind address for tailnet HTTP (never `0.0.0.0`).
pub fn resolve_tailnet_bind_ip() -> Result<IpAddr, String> {
    if let Ok(raw) = std::env::var("CDAMUX_TAILNET_BIND_ADDR") {
        return raw
            .parse()
            .map_err(|e| format!("CDAMUX_TAILNET_BIND_ADDR: {e}"));
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
}

impl LiveTailnetBackend {
    pub fn new(
        version: String,
        projects: ProjectsConfig,
        sessions: SessionsConfig,
        idle_seconds_for_needs_input: u32,
    ) -> Self {
        Self {
            version,
            projects,
            sessions,
            idle_seconds_for_needs_input,
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
        grouped_for_list(
            self.sessions.group_by,
            &self.projects,
            self.idle_seconds_for_needs_input,
        )
    }

    fn pane_tail(&self, session_id: &str) -> Result<String, TailnetApiError> {
        session_must_exist(session_id)?;
        capture_pane_text(session_id, PANE_CAPTURE_LINES)
            .map_err(|e| TailnetApiError::Tmux(e.to_string()))
    }

    fn send_line(&self, session_id: &str, line: &str, enter: bool) -> Result<(), TailnetApiError> {
        session_must_exist(session_id)?;
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
    let path = PathBuf::from(repo);
    if !path.is_absolute() {
        return Err(TailnetApiError::BadRequest(
            "repo must be an absolute path".into(),
        ));
    }
    let discovered = discover_projects(projects);
    if discovered.iter().any(|p| p.path == path) {
        return Ok(path);
    }
    if project_for_path(repo, &discovered).is_some() {
        return Ok(path);
    }
    for root in &projects.roots {
        if path.starts_with(Path::new(&root.path)) {
            return Ok(path);
        }
    }
    Err(TailnetApiError::NotFound(format!(
        "repo not under configured project roots: {repo}"
    )))
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

fn session_cwd_from_grouped(grouped: &GroupedSessions, session_id: &str) -> Option<String> {
    for group in &grouped.groups {
        for s in &group.sessions {
            if s.name == session_id {
                return Some(s.path.clone());
            }
        }
    }
    None
}

fn session_must_exist(session_id: &str) -> Result<(), TailnetApiError> {
    let sessions = list_tmux_sessions().unwrap_or_default();
    if sessions.iter().any(|s| s.name == session_id) {
        Ok(())
    } else {
        Err(TailnetApiError::NotFound(format!("session not found: {session_id}")))
    }
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
            Err(e) => {
                let status = match e {
                    TailnetApiError::NotFound(_) => 404,
                    TailnetApiError::BadRequest(_) => 400,
                    _ => 502,
                };
                return json_error(status, e);
            }
        };
        return match load_orch_graph(&repo_path) {
            Ok((graph, _)) => json_ok(200, &graph),
            Err(OrchGraphError::NotFound(_)) => json_error(
                404,
                TailnetApiError::NotFound(format!(
                    "no .orch/graph/current.json under {}",
                    repo_path.display()
                )),
            ),
            Err(OrchGraphError::Invalid { path, message }) => json_error(
                422,
                TailnetApiError::BadRequest(format!(
                    "invalid current.json at {}: {message}",
                    path.display()
                )),
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
            Err(e) => {
                let status = match e {
                    TailnetApiError::NotFound(_) => 404,
                    TailnetApiError::BadRequest(_) => 400,
                    _ => 502,
                };
                return json_error(status, e);
            }
        };
        return match load_orch_run_status(&repo_path) {
            Some(st) => json_ok(200, &st),
            None => json_error(
                404,
                TailnetApiError::NotFound(format!(
                    "no .orch/run/status.json under {}",
                    repo_path.display()
                )),
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
            Err(e) => {
                let status = match e {
                    TailnetApiError::NotFound(_) => 404,
                    TailnetApiError::BadRequest(_) => 400,
                    _ => 502,
                };
                return json_error(status, e);
            }
        };
        #[derive(Serialize)]
        struct RunRow {
            run_id: String,
            status: Option<String>,
            path: String,
        }
        let rows: Vec<RunRow> = list_orch_graph_runs(&repo_path, 20)
            .into_iter()
            .map(|r| RunRow {
                run_id: r.run_id,
                status: r.status,
                path: r.path.display().to_string(),
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
                    Err(e) => {
                        let status = match e {
                            TailnetApiError::NotFound(_) => 404,
                            _ => 502,
                        };
                        return json_error(status, e);
                    }
                }
            }
            if action == "orch-graph" && method == "GET" {
                if let Err(e) = session_must_exist(&session_id) {
                    let status = match e {
                        TailnetApiError::NotFound(_) => 404,
                        _ => 502,
                    };
                    return json_error(status, e);
                }
                let cwd = match session_cwd_from_grouped(&backend.sessions_grouped(), &session_id) {
                    Some(p) => p,
                    None => {
                        return json_error(
                            404,
                            TailnetApiError::NotFound(format!("session not found: {session_id}")),
                        );
                    }
                };
                match find_orch_graph_from_cwd(&cwd) {
                    Ok((graph, _)) => return json_ok(200, &graph),
                    Err(OrchGraphError::NotFound(_)) => {
                        return json_error(
                            404,
                            TailnetApiError::NotFound(
                                "no .orch/graph/current.json for this session cwd".into(),
                            ),
                        );
                    }
                    Err(OrchGraphError::Invalid { path, message }) => {
                        return json_error(
                            422,
                            TailnetApiError::BadRequest(format!(
                                "invalid current.json at {}: {message}",
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
                match backend.send_line(&session_id, &req.line, req.enter) {
                    Ok(()) => return json_ok(200, &SendOkPayload { ok: true }),
                    Err(e) => {
                        let status = match e {
                            TailnetApiError::NotFound(_) => 404,
                            TailnetApiError::BadRequest(_) => 400,
                            _ => 502,
                        };
                        return json_error(status, e);
                    }
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
    }

    impl TailnetApiBackend for MockBackend {
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
                }],
            }],
        }
    }

    #[test]
    fn health_returns_version_and_tmux_flag() {
        let backend = MockBackend {
            version: "0.0.1".into(),
            tmux_ok: true,
            grouped: mock_grouped(),
            pane: None,
            send_ok: true,
        };
        let resp = dispatch_tailnet_request("GET", "/v1/health", "", &backend);
        assert_eq!(resp.status, 200);
        assert!(resp.body.contains("\"tmux_reachable\":true"));
        assert!(resp.body.contains("0.0.1"));
    }

    #[test]
    fn sessions_returns_grouped_json() {
        let backend = MockBackend {
            version: "0".into(),
            tmux_ok: true,
            grouped: mock_grouped(),
            pane: None,
            send_ok: true,
        };
        let resp = dispatch_tailnet_request("GET", "/v1/sessions", "", &backend);
        assert_eq!(resp.status, 200);
        assert!(resp.body.contains("c-foo"));
    }

    #[test]
    fn pane_not_found_is_structured_404() {
        let backend = MockBackend {
            version: "0".into(),
            tmux_ok: true,
            grouped: mock_grouped(),
            pane: Some("hi".into()),
            send_ok: true,
        };
        let resp = dispatch_tailnet_request("GET", "/v1/sessions/c-missing/pane", "", &backend);
        assert_eq!(resp.status, 404);
        assert!(resp.body.contains("\"code\":\"not_found\""));
    }

    #[test]
    fn orch_graph_missing_file_is_404() {
        let backend = MockBackend {
            version: "0".into(),
            tmux_ok: true,
            grouped: mock_grouped(),
            pane: None,
            send_ok: true,
        };
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
        let backend = MockBackend {
            version: "0".into(),
            tmux_ok: true,
            grouped: mock_grouped(),
            pane: None,
            send_ok: true,
        };
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
    fn decode_path_segment_percent() {
        assert_eq!(decode_path_segment("c%2Dfoo"), "c-foo");
    }
}
