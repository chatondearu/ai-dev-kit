//! One-shot move to a single `CDAMUX_HOME` (#391, Q8): archive the legacy XDG state
//! tree and drop ghost orchestrator records. Dry-run unless `apply`; the owner runs it.

use crate::paths::cdamux_home;
use crate::session_registry;
use crate::state::{state_list, StateKind};
use std::io;
use std::path::{Path, PathBuf};

/// What `cdamux state migrate-home` does (or would do).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct HomeMigration {
    /// The single home (`$CDAMUX_HOME`).
    pub home: PathBuf,
    /// `${XDG_STATE_HOME:-~/.local/state}/cdamux` and where it is renamed to.
    pub legacy: Option<(PathBuf, PathBuf)>,
    /// Legacy agents whose tmux session still runs them: archiving is refused.
    pub legacy_live: Vec<String>,
    /// Orchestrator records of `home` to archive, with why.
    pub ghosts: Vec<(String, String)>,
}

/// The pre-#391 fallback home, whatever `CDAMUX_HOME` says.
pub fn legacy_home() -> Option<PathBuf> {
    let base = match std::env::var_os("XDG_STATE_HOME").filter(|v| !v.is_empty()) {
        Some(x) => PathBuf::from(x),
        None => PathBuf::from(std::env::var_os("HOME")?).join(".local/state"),
    };
    Some(base.join("cdamux"))
}

fn same_dir(a: &Path, b: &Path) -> bool {
    let c = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    c(a) == c(b)
}

/// Build the plan. `host_owner(session)` is the `@cdamux_id` of a live tmux session
/// (`None`: no such session, or one without an id). A dormant orchestrator (dead host,
/// run outside the legacy home) is kept: `restore` may still revive it (ADR-009). Errors when `CDAMUX_HOME` is unset (nothing to migrate to).
pub fn plan_home_migration(
    host_owner: &dyn Fn(&str) -> Option<String>,
    stamp: &str,
) -> io::Result<HomeMigration> {
    if std::env::var_os("CDAMUX_HOME").is_none_or(|v| v.is_empty()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "CDAMUX_HOME is unset: export the single home first (e.g. CDAMUX_HOME=/cda/cdamux)",
        ));
    }
    let home = cdamux_home().expect("CDAMUX_HOME set");
    let mut plan = HomeMigration { home: home.clone(), ..Default::default() };
    let legacy = legacy_home().filter(|l| l.is_dir() && !same_dir(l, &home));
    if let Some(l) = &legacy {
        plan.legacy = Some((l.clone(), l.with_file_name(format!("cdamux.archived-{stamp}"))));
        for ent in std::fs::read_dir(l.join("agents")).into_iter().flatten().flatten() {
            let Ok(text) = std::fs::read_to_string(ent.path()) else { continue };
            let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { continue };
            let (Some(id), Some(host)) = (
                v.get("id").and_then(|x| x.as_str()),
                v.get("host").and_then(|x| x.as_str()).filter(|h| !h.is_empty()),
            ) else {
                continue;
            };
            if host_owner(host).as_deref() == Some(id) {
                plan.legacy_live.push(format!("{id} on {host}"));
            }
        }
        plan.legacy_live.sort();
    }
    for (id, _, v) in state_list(StateKind::Agents) {
        if v.get("role").and_then(|r| r.as_str()) != Some("orchestrator") {
            continue;
        }
        let host = v.get("host").and_then(|h| h.as_str()).unwrap_or("");
        let owner = (!host.is_empty()).then(|| host_owner(host)).flatten();
        if owner.as_deref() == Some(id.as_str()) {
            continue; // live and its own session
        }
        let repo = v
            .get("run")
            .and_then(|r| r.as_str())
            .and_then(|run| crate::state::state_get(StateKind::Runs, run).ok().flatten())
            .and_then(|r| r.get("repo")?.as_str().map(str::to_string));
        let why = if owner.is_some() {
            Some(format!("its session {host} now runs another agent"))
        } else {
            repo.filter(|r| legacy.as_deref().is_some_and(|l| Path::new(r).starts_with(l)))
                .map(|r| format!("its run lives in the legacy home ({r})"))
        };
        if let Some(why) = why {
            plan.ghosts.push((id, why));
        }
    }
    plan.ghosts.sort();
    Ok(plan)
}

/// Apply a plan: rename the legacy tree aside (refused while one of its agents runs),
/// then archive each ghost record (and its inbox) and forget its restore record.
pub fn apply_home_migration(plan: &HomeMigration) -> io::Result<()> {
    if let Some((from, to)) = &plan.legacy {
        if !plan.legacy_live.is_empty() {
            return Err(io::Error::other(format!(
                "legacy home still runs {}; kill those sessions first",
                plan.legacy_live.join(", ")
            )));
        }
        std::fs::rename(from, to)?;
    }
    for (id, _) in &plan.ghosts {
        for rec in session_registry::load_all() {
            if rec.cdamux_id.as_deref() == Some(id.as_str()) {
                session_registry::remove(&rec.name)?;
            }
        }
        if state_list(StateKind::Agents).iter().any(|(aid, _, _)| aid == id) {
            crate::state_gc::archive_agent(id)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::env_lock;
    use crate::state::state_set;
    use serde_json::json;

    #[test]
    fn plan_finds_legacy_tree_and_ghosts_then_apply_archives() {
        let _g = env_lock();
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        let xdg = dir.path().join("xdg");
        std::fs::create_dir_all(xdg.join("cdamux/agents")).unwrap();
        std::env::set_var("CDAMUX_HOME", &home);
        std::env::set_var("XDG_STATE_HOME", &xdg);
        let legacy_global = xdg.join("cdamux/global");
        std::fs::write(
            xdg.join("cdamux/agents/a-old.json"),
            json!({"id": "a-old", "host": "c-old"}).to_string(),
        )
        .unwrap();
        // Ghost: project orchestrator whose host is reused by the live global one.
        state_set(StateKind::Agents, "a-ghost", &json!({"id": "a-ghost", "role": "orchestrator", "host": "c-global", "run": "r-g"})).unwrap();
        state_set(StateKind::Runs, "r-g", &json!({"id": "r-g", "repo": legacy_global.display().to_string()})).unwrap();
        // Dormant orchestrator (dead host, own run): kept for restore (ADR-009).
        state_set(StateKind::Agents, "a-dorm", &json!({"id": "a-dorm", "role": "orchestrator", "host": "c-dorm"})).unwrap();

        let owner = |h: &str| (h == "c-global").then(|| "a-live".to_string());
        let plan = plan_home_migration(&owner, "T").unwrap();
        assert_eq!(plan.legacy.as_ref().unwrap().1, xdg.join("cdamux.archived-T"));
        assert!(plan.legacy_live.is_empty());
        assert_eq!(plan.ghosts.iter().map(|g| g.0.as_str()).collect::<Vec<_>>(), ["a-ghost"]);

        // A legacy agent still running in its session blocks the archive.
        let busy = |h: &str| (h == "c-old").then(|| "a-old".to_string());
        let blocked = plan_home_migration(&busy, "T").unwrap();
        assert_eq!(blocked.legacy_live, ["a-old on c-old"]);
        assert!(apply_home_migration(&blocked).is_err());
        assert!(xdg.join("cdamux").is_dir());

        apply_home_migration(&plan).unwrap();
        assert!(!xdg.join("cdamux").exists());
        assert!(xdg.join("cdamux.archived-T/agents/a-old.json").is_file());
        assert!(home.join("archive/agents/a-ghost.json").is_file());
        assert!(home.join("agents/a-dorm.json").is_file());

        std::env::remove_var("XDG_STATE_HOME");
        std::env::remove_var("CDAMUX_HOME");
        let err = plan_home_migration(&owner, "T").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }
}
