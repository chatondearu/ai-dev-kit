//! PR and source-issue links of an agent (PRD-315): detected from its branch via `gh`,
//! cached in `agents/<id>.json` (`pr`, `issue`, `links_checked_at`), a manual
//! `link_override` wins. Network only here, never on a draw/refresh path (ADR-004).

use crate::issue_ref::parse_issue_number;
use crate::session_registry::{self, now_secs};
use crate::state::{state_append, state_get, StateKind};
use crate::worktree::current_branch;
use serde_json::{json, Value};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

/// How long a detection (hit, miss or `gh` failure) is trusted.
pub const LINK_TTL_SECS: u64 = 600;

/// The fields of `gh pr list --json number,url,state,title,body` we use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GhPr {
    pub number: u32,
    pub url: String,
    pub state: String,
    pub title: String,
    pub body: String,
}

/// The real `gh` runner (injectable in tests): stdout JSON, `None` when `gh` is missing,
/// offline or failing. Newest PR first, any state, so a merged PR keeps its link.
pub fn gh_pr_list(cwd: &Path, branch: &str) -> Option<String> {
    let out = Command::new("gh")
        .current_dir(cwd)
        .args([
            "pr", "list", "--head", branch, "--state", "all", "--limit", "1", "--json",
            "number,url,state,title,body",
        ])
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// First PR of the `gh pr list` JSON array; `None` for `[]` or garbage.
pub fn parse_pr_list(json: &str) -> Option<GhPr> {
    let v: Value = serde_json::from_str(json).ok()?;
    let pr = v.as_array()?.first()?;
    let s = |k: &str| pr.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
    Some(GhPr {
        number: u32::try_from(pr.get("number")?.as_u64()?).ok()?,
        url: s("url"),
        state: s("state"),
        title: s("title"),
        body: s("body"),
    })
}

/// Issue number of the first `Closes #N` / `Fixes #N` (any case, optional colon).
pub fn closes_issue(body: &str) -> Option<u32> {
    let lower = body.to_ascii_lowercase();
    ["closes", "fixes"].iter().find_map(|kw| {
        lower.match_indices(kw).find_map(|(i, _)| {
            let rest = lower[i + kw.len()..].trim_start_matches(':').trim_start();
            let digits: String = rest
                .strip_prefix('#')?
                .chars()
                .take_while(char::is_ascii_digit)
                .collect();
            digits.parse().ok()
        })
    })
}

/// `https://host/o/r/pull/12` → `https://host/o/r`.
fn repo_base(agent: &Value) -> Option<String> {
    ["pr", "issue"].iter().find_map(|k| {
        let url = agent.get(k)?.get("url")?.as_str()?;
        let (base, _) = url.rsplit_once("/pull/").or_else(|| url.rsplit_once("/issues/"))?;
        Some(base.to_string())
    })
}

fn issue_url(base: Option<&str>, n: u32) -> Value {
    base.map_or(Value::Null, |b| json!(format!("{b}/issues/{n}")))
}

/// Overrides win over detection, per field.
fn effective(agent: &Value, field: &str, detected: Value) -> Value {
    agent
        .get("link_override")
        .and_then(|o| o.get(field))
        .cloned()
        .unwrap_or(detected)
}

fn refresh_with(
    id: &str,
    cwd: &Path,
    branch: Option<String>,
    force: bool,
    now: u64,
    gh: &dyn Fn(&Path, &str) -> Option<String>,
) -> io::Result<()> {
    let cur = state_get(StateKind::Agents, id)?
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("no agent '{id}'")))?;
    let fresh = cur
        .get("links_checked_at")
        .and_then(Value::as_u64)
        .is_some_and(|t| now.saturating_sub(t) < LINK_TTL_SECS);
    // Detached HEAD / no repo: no link.
    let Some(branch) = branch.filter(|b| b != "HEAD") else {
        return Ok(());
    };
    if fresh && !force {
        return Ok(());
    }
    // `gh` failure keeps the cached links; the stamp still throttles the retries.
    let Some(out) = gh(cwd, &branch) else {
        return state_append(StateKind::Agents, id, &json!({ "links_checked_at": now }));
    };
    let pr = parse_pr_list(&out);
    let issue_n = pr
        .as_ref()
        .and_then(|p| closes_issue(&p.body))
        .or_else(|| parse_issue_number(&branch));
    let base = pr
        .as_ref()
        .and_then(|p| p.url.rsplit_once("/pull/").map(|(b, _)| b.to_string()))
        .or_else(|| repo_base(&cur));
    let pr_val = pr.map_or(Value::Null, |p| {
        json!({"number": p.number, "url": p.url, "state": p.state, "title": p.title, "checked_at": now})
    });
    let issue_val = issue_n.map_or(Value::Null, |n| {
        json!({"number": n, "url": issue_url(base.as_deref(), n)})
    });
    state_append(
        StateKind::Agents,
        id,
        &json!({
            "pr": effective(&cur, "pr", pr_val),
            "issue": effective(&cur, "issue", issue_val),
            "links_checked_at": now,
        }),
    )
}

/// Detect the PR / issue of agent `id` from the branch checked out in `cwd`, unless the
/// cache is younger than [`LINK_TTL_SECS`] (`force` skips that). Never errors on `gh`.
pub fn refresh_links(id: &str, cwd: &Path, force: bool) -> io::Result<()> {
    refresh_with(id, cwd, current_branch(cwd), force, now_secs(), &gh_pr_list)
}

/// Working directory of agent `id`: its restore record, else a `cwd` state field.
pub fn agent_cwd(id: &str) -> Option<PathBuf> {
    session_registry::load_all()
        .into_iter()
        .find(|r| r.cdamux_id.as_deref() == Some(id))
        .map(|r| r.cwd)
        .or_else(|| {
            let v = state_get(StateKind::Agents, id).ok()??;
            Some(PathBuf::from(v.get("cwd")?.as_str()?))
        })
}

/// A PR or issue given as `N` or as its URL (`.../pull/N`, `.../issues/N`).
pub fn parse_link_ref(s: &str) -> Option<(u32, Option<String>)> {
    let s = s.trim().trim_start_matches('#');
    if let Ok(n) = s.parse() {
        return Some((n, None));
    }
    let tail = s.rsplit_once("/pull/").or_else(|| s.rsplit_once("/issues/"))?.1;
    let digits: String = tail.chars().take_while(char::is_ascii_digit).collect();
    Some((digits.parse().ok()?, Some(s.to_string())))
}

/// Pin the PR and/or issue of agent `id`; detection no longer replaces a pinned field.
/// A bare number borrows the repo URL of the links already known.
pub fn set_link_override(
    id: &str,
    pr: Option<(u32, Option<String>)>,
    issue: Option<(u32, Option<String>)>,
) -> io::Result<()> {
    let cur = state_get(StateKind::Agents, id)?
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("no agent '{id}'")))?;
    let base = repo_base(&cur);
    let mut ov = cur
        .get("link_override")
        .filter(|o| o.is_object())
        .cloned()
        .unwrap_or_else(|| json!({}));
    let mut patch = json!({});
    for (field, kind, given) in [("pr", "pull", pr), ("issue", "issues", issue)] {
        let Some((n, url)) = given else { continue };
        let url = url.or_else(|| base.as_ref().map(|b| format!("{b}/{kind}/{n}")));
        let link = json!({ "number": n, "url": url });
        ov[field] = link.clone();
        patch[field] = link;
    }
    patch["link_override"] = ov;
    state_append(StateKind::Agents, id, &patch)
}

/// Drop the override and the cached links; the next refresh detects again.
pub fn clear_links(id: &str) -> io::Result<()> {
    state_append(
        StateKind::Agents,
        id,
        &json!({ "link_override": null, "pr": null, "issue": null, "links_checked_at": null }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::env_lock;
    use std::cell::Cell;

    const PR_JSON: &str = r#"[{"number":12,"url":"https://github.com/o/r/pull/12","state":"OPEN","title":"feat: x","body":"Why\n\nCloses #7"}]"#;

    #[test]
    fn parses_closes_and_pr_list() {
        assert_eq!(closes_issue("done.\nFixes: #31 and more"), Some(31));
        assert_eq!(closes_issue("CLOSES #5"), Some(5));
        assert_eq!(closes_issue("closes the loop, see #9"), None);
        assert_eq!(closes_issue("no link"), None);
        let pr = parse_pr_list(PR_JSON).unwrap();
        assert_eq!((pr.number, pr.state.as_str()), (12, "OPEN"));
        assert!(parse_pr_list("[]").is_none());
        assert!(parse_pr_list("not json").is_none());
        assert_eq!(
            parse_link_ref("https://github.com/o/r/pull/12"),
            Some((12, Some("https://github.com/o/r/pull/12".into())))
        );
        assert_eq!(parse_link_ref("#4"), Some((4, None)));
        assert_eq!(parse_link_ref("abc"), None);
    }

    #[test]
    fn detect_cache_override_and_clear() {
        let _g = env_lock();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("CDAMUX_HOME", dir.path());
        let id = "a-link1";
        state_append(StateKind::Agents, id, &json!({"id": id})).unwrap();
        let calls = Cell::new(0);
        let out = Cell::new(Some(PR_JSON.to_string()));
        let gh = |_: &Path, _: &str| {
            calls.set(calls.get() + 1);
            out.take().inspect(|s| out.set(Some(s.clone())))
        };
        let cwd = Path::new("/tmp");
        let run = |branch: &str, force, now| {
            refresh_with(id, cwd, Some(branch.into()), force, now, &gh).unwrap();
            state_get(StateKind::Agents, id).unwrap().unwrap()
        };

        // Detection: PR from gh, issue from `Closes #7` (wins over the branch number).
        let a = run("feat/177-x", false, 1000);
        assert_eq!(a["pr"]["number"], 12);
        assert_eq!(a["issue"]["number"], 7);
        assert_eq!(a["issue"]["url"], "https://github.com/o/r/issues/7");

        // Cache: no second call within the TTL, one after it or when forced.
        run("feat/177-x", false, 1000 + LINK_TTL_SECS - 1);
        assert_eq!(calls.get(), 1);
        run("feat/177-x", true, 1001);
        run("feat/177-x", false, 1001 + LINK_TTL_SECS);
        assert_eq!(calls.get(), 3);

        // gh failure keeps the cached links.
        out.set(None);
        let a = run("feat/177-x", true, 2000);
        assert_eq!(a["pr"]["number"], 12);
        assert_eq!(a["links_checked_at"], 2000);

        // Override wins over a later detection, per field.
        set_link_override(id, Some((99, None)), None).unwrap();
        out.set(Some(PR_JSON.to_string()));
        let a = run("feat/177-x", true, 3000);
        assert_eq!(a["pr"]["number"], 99);
        assert_eq!(a["pr"]["url"], "https://github.com/o/r/pull/99");
        assert_eq!(a["issue"]["number"], 7);

        // Clear drops everything; no PR → issue from the branch name, no URL.
        clear_links(id).unwrap();
        out.set(Some("[]".into()));
        let a = run("worker/x/295-slug", true, 4000);
        assert_eq!(a["pr"], Value::Null);
        assert_eq!(a["issue"]["number"], 295);
        assert_eq!(a["issue"]["url"], Value::Null);

        // Detached HEAD: untouched.
        let before = calls.get();
        run("HEAD", true, 5000);
        assert_eq!(calls.get(), before);

        std::env::remove_var("CDAMUX_HOME");
    }
}
