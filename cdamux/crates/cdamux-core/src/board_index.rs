//! Global foam-board resume index under `$CDAMUX_HOME/board-index.json` (ADR-016).
//!
//! Cache rebuilt from each discovered project's `foam/board`. Git is the sync;
//! this file is never source of truth. Atomic write (tmp + rename, ADR-011).

use crate::state::write_atomic;
use crate::Project;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

/// Filename under `$CDAMUX_HOME`.
pub const BOARD_INDEX_FILE: &str = "board-index.json";

/// Local directory names omitted from the index (Beamy clones that may lack
/// the org in the remote). ADR-016 skip policy is org/remote; do not add
/// host filesystem paths here.
const SKIP_NAMES: &[&str] = &[
    "beamy-v2",
    "data-pipelines",
    "dev-setup",
    "web-extension",
    "windows-agent",
];

const BOARD_DIR: &str = "foam/board";

/// Rebuild options. Default: `git fetch` + local foam; no `git pull`.
#[derive(Debug, Clone, Copy)]
pub struct RefreshOptions {
    /// `git pull --ff-only` per included project. Off unless a documented flag.
    pub pull: bool,
    /// `git fetch` before reading local foam. Tests that must stay offline set false.
    pub fetch: bool,
    /// Best-effort `gh pr list`. A failure never aborts the rest.
    pub gh: bool,
}

impl Default for RefreshOptions {
    fn default() -> Self {
        Self {
            pull: false,
            fetch: true,
            gh: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoardIndex {
    pub generated_at: String,
    pub projects: Vec<BoardProject>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoardProject {
    pub name: String,
    pub path: String,
    pub ready: Vec<BoardCard>,
    pub in_progress: Vec<BoardCard>,
    pub in_review: Vec<BoardCard>,
    pub blocked_by: Vec<BoardCard>,
    pub open_prs: Vec<OpenPr>,
    /// In-progress / in-review cards whose PR is merged (report only; the fix is
    /// `foam-board.sh reconcile --pr`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub drift: Vec<BoardDrift>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoardCard {
    pub slug: String,
    pub title: String,
    pub status: String,
    pub priority: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub github_issue: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocked_by: Vec<String>,
    /// Frontmatter `updated` (YYYY-MM-DD): a PR merged before it is not this card's.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub updated: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenPr {
    pub number: u32,
    pub url: String,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoardDrift {
    pub slug: String,
    pub status: String,
    pub pr: u32,
}

/// Merged PR as `gh pr list --state merged` reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct MergedPr {
    number: u32,
    head: String,
    /// `mergedAt` day (YYYY-MM-DD).
    merged: String,
    /// Issues it closes in this repository only.
    closes: Vec<u64>,
}

/// `$CDAMUX_HOME/board-index.json`.
pub fn board_index_file(home: &Path) -> PathBuf {
    home.join(BOARD_INDEX_FILE)
}

#[cfg(test)]
fn is_skipped_project(name: &str, path: &Path, remotes: &[String]) -> bool {
    skipped_name_or_path(name, path) || remotes.iter().any(|r| remote_is_beamy(r))
}

fn skipped_name_or_path(name: &str, _path: &Path) -> bool {
    SKIP_NAMES.iter().any(|n| *n == name)
}

fn remote_is_beamy(remote: &str) -> bool {
    let lower = remote.to_ascii_lowercase();
    lower.contains("beamysoftware/")
}

/// Rebuild and atomically write `$home/board-index.json`. Always succeeds for
/// missing foam (those projects get `skip`); I/O on the index file can still fail.
pub fn refresh_board_index(
    home: &Path,
    projects: &[Project],
    opts: &RefreshOptions,
) -> io::Result<BoardIndex> {
    let index = build_board_index(projects, opts);
    write_board_index(home, &index)?;
    Ok(index)
}

fn write_board_index(home: &Path, index: &BoardIndex) -> io::Result<()> {
    let path = board_index_file(home);
    let mut bytes = serde_json::to_vec_pretty(index)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    bytes.push(b'\n');
    write_atomic(&path, &bytes)
}

fn build_board_index(projects: &[Project], opts: &RefreshOptions) -> BoardIndex {
    let mut out = Vec::with_capacity(projects.len());
    for project in projects {
        if skipped_name_or_path(&project.name, &project.path) {
            continue;
        }
        let remotes = git_remotes(&project.path);
        if remotes.iter().any(|r| remote_is_beamy(r)) {
            continue;
        }
        out.push(scan_project(project, &remotes, opts));
    }
    BoardIndex {
        generated_at: unix_rfc3339_utc(now_secs()),
        projects: out,
    }
}

fn scan_project(project: &Project, remotes: &[String], opts: &RefreshOptions) -> BoardProject {
    let mut row = empty_project(&project.name, &project.path);
    if !project.path.join(".git").exists() {
        row.skip = Some("not a git repository".into());
        return row;
    }
    if opts.fetch {
        git_fetch(&project.path);
    }
    if opts.pull {
        if let Err(e) = git_pull_ff(&project.path) {
            row.error = Some(e);
        }
    }
    let foam = project.path.join("foam");
    let board = project.path.join(BOARD_DIR);
    if !foam.is_dir() {
        row.skip = Some("missing foam".into());
        return row;
    }
    if !board.is_dir() {
        row.skip = Some("missing foam/board".into());
        return row;
    }
    let cards = read_cards(&board);
    for card in &cards {
        match card.status.as_str() {
            "ready" => row.ready.push(card.clone()),
            "in-progress" => row.in_progress.push(card.clone()),
            "in-review" => row.in_review.push(card.clone()),
            _ => {}
        }
        if !card.blocked_by.is_empty() && card.status != "done" {
            row.blocked_by.push(card.clone());
        }
    }
    if opts.gh && should_query_gh(&cards, remotes) {
        row.open_prs = gh_open_prs(&project.path);
        if !row.in_progress.is_empty() || !row.in_review.is_empty() {
            row.drift = board_drift(&cards, &gh_merged_prs(&project.path));
        }
    }
    row
}

/// Same rule as `foam-board.sh reconcile`: an in-progress|in-review card with a PR merged
/// into the default branch (`merged` holds only those) on or after its `updated` day,
/// whose head is `feat/<id>-issue-<id>` (id = github_issue, else slug) or which closes
/// its github_issue in this repository.
fn board_drift(cards: &[BoardCard], merged: &[MergedPr]) -> Vec<BoardDrift> {
    cards
        .iter()
        .filter(|c| matches!(c.status.as_str(), "in-progress" | "in-review"))
        .filter_map(|c| {
            let id = c.github_issue.map_or_else(|| c.slug.clone(), |n| n.to_string());
            let branch = format!("feat/{id}-issue-{id}");
            let pr = merged.iter().find(|p| {
                p.merged.as_str() >= c.updated.as_str()
                    && (p.head == branch || c.github_issue.is_some_and(|n| p.closes.contains(&n)))
            })?;
            Some(BoardDrift {
                slug: c.slug.clone(),
                status: c.status.clone(),
                pr: pr.number,
            })
        })
        .collect()
}

fn empty_project(name: &str, path: &Path) -> BoardProject {
    BoardProject {
        name: name.to_string(),
        path: path.to_string_lossy().into_owned(),
        ready: Vec::new(),
        in_progress: Vec::new(),
        in_review: Vec::new(),
        blocked_by: Vec::new(),
        open_prs: Vec::new(),
        drift: Vec::new(),
        skip: None,
        error: None,
    }
}

fn should_query_gh(cards: &[BoardCard], remotes: &[String]) -> bool {
    cards.iter().any(|c| c.github_issue.is_some())
        || remotes.iter().any(|r| {
            let l = r.to_ascii_lowercase();
            l.contains("github.com") || l.contains("github:")
        })
}

fn read_cards(board: &Path) -> Vec<BoardCard> {
    let Ok(entries) = fs::read_dir(board) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    paths.sort();
    let mut cards = Vec::new();
    for path in paths {
        if let Some(card) = parse_card_file(&path) {
            cards.push(card);
        }
    }
    cards
}

fn is_card_filename(name: &str) -> bool {
    if !name.ends_with(".md") && !name.ends_with(".MD") {
        return false;
    }
    if name.eq_ignore_ascii_case("README.md") {
        return false;
    }
    !name.starts_with('_')
}

fn parse_card_file(path: &Path) -> Option<BoardCard> {
    let name = path.file_name()?.to_str()?;
    if !is_card_filename(name) {
        return None;
    }
    if !path.is_file() {
        return None;
    }
    let raw = fs::read_to_string(path).ok()?;
    let parsed = parse_frontmatter(&raw)?;
    let slug = name
        .strip_suffix(".md")
        .or_else(|| name.strip_suffix(".MD"))
        .unwrap_or(name)
        .to_string();
    let title = parsed.title.unwrap_or_else(|| slug.clone());
    Some(BoardCard {
        slug,
        title,
        status: parsed.status,
        priority: parsed.priority,
        github_issue: parsed.github_issue,
        blocked_by: parsed.blocked_by,
        updated: parsed.updated,
    })
}

struct Frontmatter {
    title: Option<String>,
    status: String,
    priority: String,
    github_issue: Option<u64>,
    blocked_by: Vec<String>,
    updated: String,
}

fn parse_frontmatter(raw: &str) -> Option<Frontmatter> {
    let body = raw.trim_start_matches('\u{feff}').trim_start();
    let rest = body.strip_prefix("---")?;
    let rest = rest.strip_prefix('\r').unwrap_or(rest).strip_prefix('\n')?;
    let end = rest.find("\n---")?;
    let yaml = &rest[..end];
    let mut title = None;
    let mut status = None;
    let mut priority = None;
    let mut updated = None;
    let mut github_issue = None;
    let mut blocked_by = Vec::new();
    let mut in_blocked = false;
    for line in yaml.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if in_blocked {
            if let Some(item) = trimmed.strip_prefix("- ") {
                let v = unquote(item.trim());
                if !v.is_empty() {
                    blocked_by.push(v);
                }
                continue;
            }
            in_blocked = false;
        }
        let Some((key, value)) = trimmed.split_once(':') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        match key {
            "title" => title = Some(unquote(value)).filter(|s| !s.is_empty()),
            "status" => status = Some(unquote(value)),
            "priority" => priority = Some(unquote(value)),
            "updated" => updated = Some(unquote(value)),
            "github_issue" => github_issue = unquote(value).parse().ok(),
            "blocked_by" => {
                if value.is_empty() {
                    in_blocked = true;
                } else {
                    blocked_by = parse_yaml_list(value);
                }
            }
            _ => {}
        }
    }
    let status = status.filter(|s| is_status(s))?;
    let priority = priority.filter(|p| is_priority(p))?;
    let updated = updated.filter(|u| is_updated(u))?;
    Some(Frontmatter {
        title,
        status,
        priority,
        github_issue,
        blocked_by,
        updated,
    })
}

fn is_status(s: &str) -> bool {
    matches!(
        s,
        "backlog" | "ready" | "in-progress" | "in-review" | "done"
    )
}

fn is_priority(s: &str) -> bool {
    matches!(s, "P0" | "P1" | "P2")
}

fn is_updated(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter().enumerate().all(|(i, c)| {
            if i == 4 || i == 7 {
                true
            } else {
                c.is_ascii_digit()
            }
        })
}

fn unquote(s: &str) -> String {
    let s = s.trim();
    if s.len() >= 2 {
        let b = s.as_bytes();
        if (b[0] == b'"' && b[s.len() - 1] == b'"') || (b[0] == b'\'' && b[s.len() - 1] == b'\'') {
            return s[1..s.len() - 1].to_string();
        }
    }
    s.to_string()
}

fn parse_yaml_list(value: &str) -> Vec<String> {
    let s = value.trim();
    if s == "[]" {
        return Vec::new();
    }
    let inner = s
        .strip_prefix('[')
        .and_then(|t| t.strip_suffix(']'))
        .unwrap_or(s);
    inner
        .split(',')
        .map(|p| unquote(p.trim()))
        .filter(|p| !p.is_empty())
        .collect()
}

fn parse_open_prs(json: &str) -> Vec<OpenPr> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    let Some(arr) = v.as_array() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for pr in arr {
        let Some(number) = pr
            .get("number")
            .and_then(|n| n.as_u64())
            .and_then(|n| u32::try_from(n).ok())
        else {
            continue;
        };
        out.push(OpenPr {
            number,
            url: pr
                .get("url")
                .and_then(|u| u.as_str())
                .unwrap_or("")
                .to_string(),
            title: pr
                .get("title")
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .to_string(),
        });
    }
    out.sort_by_key(|p| p.number);
    out
}

/// PRs merged into `default` (base filter) from `gh pr list` JSON; closing issues kept
/// only when they live in `slug` (owner/name, case-insensitive).
fn parse_merged_prs(json: &str, default: &str, slug: &str) -> Vec<MergedPr> {
    let Ok(serde_json::Value::Array(arr)) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    let same_repo = |i: &serde_json::Value| {
        let r = i.get("repository");
        let owner = r.and_then(|r| r.pointer("/owner/login")).and_then(|v| v.as_str());
        let name = r.and_then(|r| r.get("name")).and_then(|v| v.as_str());
        matches!((owner, name), (Some(o), Some(n)) if format!("{o}/{n}").eq_ignore_ascii_case(slug))
    };
    arr.iter()
        .filter(|pr| pr.get("baseRefName").and_then(|b| b.as_str()) == Some(default))
        .filter_map(|pr| {
            Some(MergedPr {
                number: u32::try_from(pr.get("number")?.as_u64()?).ok()?,
                head: pr.get("headRefName")?.as_str()?.to_string(),
                merged: pr.get("mergedAt")?.as_str()?.get(..10)?.to_string(),
                closes: pr
                    .get("closingIssuesReferences")
                    .and_then(|v| v.as_array())
                    .map(|a| {
                        a.iter()
                            .filter(|i| same_repo(i))
                            .filter_map(|i| i.get("number")?.as_u64())
                            .collect()
                    })
                    .unwrap_or_default(),
            })
        })
        .collect()
}

fn git_cmd(repo: &Path, args: &[&str]) -> Command {
    let mut cmd = Command::new("git");
    cmd.current_dir(repo)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null());
    cmd
}

fn git_remotes(repo: &Path) -> Vec<String> {
    let out = git_cmd(repo, &["remote", "-v"]).output();
    let Ok(out) = out else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| line.split_whitespace().nth(1).map(str::to_string))
        .collect()
}

fn git_fetch(repo: &Path) {
    let _ = git_cmd(repo, &["fetch", "--quiet"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

fn git_pull_ff(repo: &Path) -> Result<(), String> {
    let out = git_cmd(repo, &["pull", "--ff-only", "--quiet"])
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        let msg = err.trim();
        Err(if msg.is_empty() {
            "git pull --ff-only failed".into()
        } else {
            msg.to_string()
        })
    }
}

fn gh_open_prs(repo: &Path) -> Vec<OpenPr> {
    let out = Command::new("gh")
        .current_dir(repo)
        .args([
            "pr",
            "list",
            "--state",
            "open",
            "--limit",
            "50",
            "--json",
            "number,url,title",
        ])
        .env("GH_PROMPT_DISABLED", "1")
        .stdin(Stdio::null())
        .output();
    let Ok(out) = out else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    parse_open_prs(&String::from_utf8_lossy(&out.stdout))
}

fn gh_json(repo: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("gh")
        .current_dir(repo)
        .args(args)
        .env("GH_PROMPT_DISABLED", "1")
        .stdin(Stdio::null())
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Best-effort; a failure means no drift is reported (never a write).
fn gh_merged_prs(repo: &Path) -> Vec<MergedPr> {
    let Some(meta) = gh_json(repo, &["repo", "view", "--json", "nameWithOwner,defaultBranchRef"]) else {
        return Vec::new();
    };
    let Ok(meta) = serde_json::from_str::<serde_json::Value>(&meta) else {
        return Vec::new();
    };
    let (Some(slug), Some(default)) = (
        meta.get("nameWithOwner").and_then(|v| v.as_str()),
        meta.pointer("/defaultBranchRef/name").and_then(|v| v.as_str()),
    ) else {
        return Vec::new();
    };
    let args = [
        "pr",
        "list",
        "--state",
        "merged",
        "--base",
        default,
        "--limit",
        "200",
        "--json",
        "number,headRefName,baseRefName,mergedAt,closingIssuesReferences",
    ];
    gh_json(repo, &args)
        .map(|j| parse_merged_prs(&j, default, slug))
        .unwrap_or_default()
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// UTC civil datetime from Unix seconds (Howard Hinnant `civil_from_days`).
fn unix_rfc3339_utc(secs: u64) -> String {
    let days = (secs / 86400) as i64;
    let rem = secs % 86400;
    let h = rem / 3600;
    let m = (rem % 3600) / 60;
    let s = rem % 60;
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}T{h:02}:{m:02}:{s:02}Z")
}

fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m as u32, d as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn init_git(dir: &Path) {
        fs::create_dir_all(dir).unwrap();
        assert!(Command::new("git")
            .args(["-C", dir.to_str().unwrap(), "init", "-b", "main"])
            .status()
            .unwrap()
            .success());
    }

    fn write_card(dir: &Path, name: &str, body: &str) {
        fs::create_dir_all(dir.join(BOARD_DIR)).unwrap();
        fs::write(dir.join(BOARD_DIR).join(name), body).unwrap();
    }

    fn project(name: &str, path: &Path) -> Project {
        Project {
            name: name.into(),
            path: path.to_path_buf(),
            root_label: "t".into(),
        }
    }

    fn offline() -> RefreshOptions {
        RefreshOptions {
            pull: false,
            fetch: false,
            gh: false,
        }
    }

    #[test]
    fn drift_is_in_flight_card_with_merged_pr() {
        let card = |slug: &str, status: &str, gi: Option<u64>| BoardCard {
            slug: slug.into(),
            title: slug.into(),
            status: status.into(),
            priority: "P1".into(),
            github_issue: gi,
            blocked_by: Vec::new(),
            updated: "2026-11-10".into(),
        };
        let merged = parse_merged_prs(
            r#"[{"number":7,"headRefName":"worker/r/x","baseRefName":"main","mergedAt":"2026-11-12T10:00:00Z",
                 "closingIssuesReferences":[{"number":50,"repository":{"name":"r","owner":{"login":"O"}}}]},
                {"number":8,"headRefName":"feat/foam-only-issue-foam-only","baseRefName":"main","mergedAt":"2026-11-10T09:00:00Z","closingIssuesReferences":[]},
                {"number":9,"headRefName":"feat/77-issue-77","baseRefName":"main","mergedAt":"2026-11-12T10:00:00Z"},
                {"number":10,"headRefName":"worker/r/y","baseRefName":"main","mergedAt":"2026-11-12T10:00:00Z",
                 "closingIssuesReferences":[{"number":60,"repository":{"name":"repo","owner":{"login":"other"}}}]},
                {"number":11,"headRefName":"feat/61-issue-61","baseRefName":"main","mergedAt":"2026-11-01T10:00:00Z"},
                {"number":12,"headRefName":"feat/62-issue-62","baseRefName":"release","mergedAt":"2026-11-12T10:00:00Z"},
                {"oops":1}]"#,
            "main",
            "o/r",
        );
        assert_eq!(merged.len(), 5, "release base and junk dropped");
        let cards = vec![
            card("50-closed", "in-review", Some(50)),
            card("foam-only", "in-progress", None),
            card("60-other-repo", "in-review", Some(60)),
            card("61-old-pr", "in-progress", Some(61)),
            card("62-release", "in-review", Some(62)),
            card("77-ready", "ready", Some(77)),
            card("78-done", "done", Some(78)),
        ];
        let got: Vec<(String, u32)> = board_drift(&cards, &merged)
            .into_iter()
            .map(|d| (d.slug, d.pr))
            .collect();
        assert_eq!(
            got,
            vec![("50-closed".to_string(), 7), ("foam-only".to_string(), 8)]
        );
        assert!(parse_merged_prs("not json", "main", "o/r").is_empty());
    }

    #[test]
    fn skip_filter_names_paths_and_remotes() {
        assert!(is_skipped_project(
            "beamy-v2",
            Path::new("/src/beamy-v2"),
            &[]
        ));
        assert!(!is_skipped_project(
            "app",
            Path::new("/src/app"),
            &[]
        ));
        assert!(is_skipped_project(
            "kit",
            Path::new("/src/kit"),
            &["git@github.com:beamysoftware/app.git".into()]
        ));
        assert!(is_skipped_project(
            "kit",
            Path::new("/src/kit"),
            &["https://github.com/BeamySoftware/app.git".into()]
        ));
        assert!(!is_skipped_project(
            "ai-dev-kit",
            Path::new("/src/ai-dev-kit"),
            &[]
        ));
        assert!(!is_skipped_project(
            "ai-dev-kit",
            Path::new("/src/ai-dev-kit"),
            &["https://github.com/chatondearu/ai-dev-kit.git".into()]
        ));
    }

    #[test]
    fn parse_frontmatter_adr_example() {
        let raw = r#"---
title: "feat(cdamux): global foam-board resume index"
status: ready
priority: P1
updated: 2026-11-19
github_issue: 407
blocked_by: [404]
---

body
"#;
        let fm = parse_frontmatter(raw).unwrap();
        assert_eq!(
            fm.title.as_deref(),
            Some("feat(cdamux): global foam-board resume index")
        );
        assert_eq!(fm.status, "ready");
        assert_eq!(fm.priority, "P1");
        assert_eq!(fm.github_issue, Some(407));
        assert_eq!(fm.blocked_by, ["404"]);
    }

    #[test]
    fn parse_frontmatter_skips_incomplete_and_template() {
        assert!(parse_frontmatter("no fence").is_none());
        assert!(parse_frontmatter("---\nstatus: ready\n---\n").is_none());
        assert!(
            parse_frontmatter("---\nstatus: ready\npriority: P1\nupdated: 19-11-2026\n---\n")
                .is_none()
        );
        assert!(
            parse_frontmatter("---\nstatus: wip\npriority: P1\nupdated: 2026-11-19\n---\n")
                .is_none()
        );
        let blocked = parse_frontmatter(
            "---\nstatus: in-progress\npriority: P0\nupdated: 2026-11-19\nblocked_by:\n  - 404\n  - other-slug\n---\n",
        )
        .unwrap();
        assert_eq!(blocked.blocked_by, ["404", "other-slug"]);
    }

    #[test]
    fn parse_card_file_skips_readme_and_underscore() {
        let tmp = tempdir().unwrap();
        let board = tmp.path().join(BOARD_DIR);
        fs::create_dir_all(&board).unwrap();
        fs::write(
            board.join("README.md"),
            "---\nstatus: ready\npriority: P1\nupdated: 2026-11-19\n---\n",
        )
        .unwrap();
        fs::write(
            board.join("_template.md"),
            "---\nstatus: ready\npriority: P1\nupdated: 2026-11-19\n---\n",
        )
        .unwrap();
        fs::write(
            board.join("407-index.md"),
            "---\nstatus: ready\npriority: P1\nupdated: 2026-11-19\n---\n",
        )
        .unwrap();
        assert!(parse_card_file(&board.join("README.md")).is_none());
        assert!(parse_card_file(&board.join("_template.md")).is_none());
        let card = parse_card_file(&board.join("407-index.md")).unwrap();
        assert_eq!(card.slug, "407-index");
        assert_eq!(card.title, "407-index");
        assert_eq!(card.status, "ready");
    }

    #[test]
    fn unix_rfc3339_known_instants() {
        assert_eq!(unix_rfc3339_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(unix_rfc3339_utc(86400), "1970-01-02T00:00:00Z");
        assert_eq!(unix_rfc3339_utc(1_795_046_400), "2026-11-19T00:00:00Z");
    }

    #[test]
    fn parse_open_prs_best_effort() {
        let prs = parse_open_prs(
            r#"[{"number":2,"url":"https://example/2","title":"b"},{"number":1,"url":"https://example/1","title":"a"}]"#,
        );
        assert_eq!(prs[0].number, 1);
        assert_eq!(prs[1].number, 2);
        assert!(parse_open_prs("not-json").is_empty());
        assert!(parse_open_prs("{}").is_empty());
    }

    #[test]
    fn refresh_temp_repos_skip_vs_cards_beamy_absent() {
        let tmp = tempdir().unwrap();
        let home = tmp.path().join("home");
        let kit = tmp.path().join("kit");
        let plain = tmp.path().join("plain");
        let beamy = tmp.path().join("beamy-v2");
        let remote_beamy = tmp.path().join("ext");
        init_git(&kit);
        init_git(&plain);
        init_git(&beamy);
        init_git(&remote_beamy);
        assert!(Command::new("git")
            .args([
                "-C",
                remote_beamy.to_str().unwrap(),
                "remote",
                "add",
                "origin",
                "git@github.com:beamysoftware/ext.git",
            ])
            .status()
            .unwrap()
            .success());

        write_card(
            &kit,
            "407-index.md",
            r#"---
title: "feat(cdamux): index"
status: ready
priority: P1
updated: 2026-11-19
github_issue: 407
blocked_by: [404]
---
"#,
        );
        write_card(
            &kit,
            "10-wip.md",
            r#"---
title: "wip"
status: in-progress
priority: P0
updated: 2026-11-19
---
"#,
        );
        write_card(
            &kit,
            "11-review.md",
            r#"---
title: "review"
status: in-review
priority: P2
updated: 2026-11-19
---
"#,
        );
        write_card(
            &beamy,
            "1-secret.md",
            "---\nstatus: ready\npriority: P0\nupdated: 2026-11-19\n---\n",
        );

        let projects = vec![
            project("kit", &kit),
            project("plain", &plain),
            project("beamy-v2", &beamy),
            project("ext", &remote_beamy),
        ];
        let index = refresh_board_index(&home, &projects, &offline()).unwrap();
        let names: Vec<&str> = index.projects.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["kit", "plain"]);
        assert!(!index
            .projects
            .iter()
            .any(|p| p.name.contains("beamy") || p.path.contains("beamy-v2") || p.name == "ext"));

        let kit_row = &index.projects[0];
        assert!(kit_row.skip.is_none());
        assert_eq!(kit_row.ready.len(), 1);
        assert_eq!(kit_row.ready[0].github_issue, Some(407));
        assert_eq!(kit_row.in_progress.len(), 1);
        assert_eq!(kit_row.in_review.len(), 1);
        assert_eq!(kit_row.blocked_by.len(), 1);
        assert_eq!(kit_row.blocked_by[0].blocked_by, ["404"]);

        let plain_row = &index.projects[1];
        assert_eq!(plain_row.skip.as_deref(), Some("missing foam"));
        assert!(plain_row.ready.is_empty());

        let path = board_index_file(&home);
        assert!(path.is_file());
        let raw = fs::read_to_string(&path).unwrap();
        let parsed: BoardIndex = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed.projects.len(), 2);

        let again = refresh_board_index(&home, &projects, &offline()).unwrap();
        let mut a = serde_json::to_value(&index).unwrap();
        let mut b = serde_json::to_value(&again).unwrap();
        a["generated_at"] = serde_json::Value::Null;
        b["generated_at"] = serde_json::Value::Null;
        assert_eq!(a, b);

        let no_board = tmp.path().join("noboard");
        init_git(&no_board);
        fs::create_dir_all(no_board.join("foam")).unwrap();
        let skipped = build_board_index(&[project("noboard", &no_board)], &offline());
        assert_eq!(
            skipped.projects[0].skip.as_deref(),
            Some("missing foam/board")
        );
    }
}
