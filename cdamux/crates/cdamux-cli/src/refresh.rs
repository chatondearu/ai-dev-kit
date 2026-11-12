//! Refresh scheduling for the dashboard loop: one interval per source and an
//! mtime-gated cache for `$CDAMUX_HOME` runs/agents, so the 100 ms input poll
//! never does I/O. Every method takes `now` so tests drive a fake clock.

use cdamux_core::{
    list_live_orch_runs, list_orch_graph_runs, load_orch_run_status, orch_agents_dir,
    orch_graph_runs_dir, session_registry, session_subagents, ChildRow, OrchGraphRunSummary,
    OrchRunStatusFile, OrchWorkerStatus,
};
use session_registry::SessionRecord;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

/// Fires once per `every`; due immediately until first `mark`.
pub(crate) struct Interval {
    every: Duration,
    last: Option<Instant>,
}

impl Interval {
    pub(crate) fn new(every: Duration) -> Self {
        Self { every, last: None }
    }

    pub(crate) fn due(&self, now: Instant) -> bool {
        self.last
            .is_none_or(|t| now.saturating_duration_since(t) >= self.every)
    }

    pub(crate) fn mark(&mut self, now: Instant) {
        self.last = Some(now);
    }
}

/// What the preview sidecar must do this tick (tmux is only touched for a session row).
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Sidecar {
    Idle,
    /// Selection changed: full load (capture-pane + chrome).
    Load,
    /// Same session: cheap periodic refresh.
    Poll,
}

pub(crate) fn sidecar_step(
    on_session: bool,
    changed: bool,
    preview_visible: bool,
    iv: &mut Interval,
    now: Instant,
) -> Sidecar {
    if !on_session {
        return Sidecar::Idle;
    }
    if changed {
        iv.mark(now);
        Sidecar::Load
    } else if preview_visible && iv.due(now) {
        iv.mark(now);
        Sidecar::Poll
    } else {
        Sidecar::Idle
    }
}

/// Orch runs of one repo: live (newest first) and the last few archived.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct RepoRuns {
    pub(crate) live: Vec<OrchGraphRunSummary>,
    pub(crate) recent: Vec<OrchGraphRunSummary>,
}

/// mtime of `$CDAMUX_HOME/runs` (shared across repos; list filters by `repo` field).
type RunsStamp = Option<SystemTime>;

fn mtime(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// Orch workers per repo, re-read only when the file's mtime moved, orch runs per repo
/// (same rule on the graph dirs), CLI sub-agent rows per tmux session (ledger,
/// mtime-cached in core), and the restore registry (same rule on its dir), on the same interval.
pub(crate) struct StatusCache {
    iv: Interval,
    entries: HashMap<PathBuf, (Option<SystemTime>, OrchRunStatusFile)>,
    runs_iv: Interval,
    runs: HashMap<PathBuf, (RunsStamp, RepoRuns)>,
    sub_iv: Interval,
    subagents: HashMap<String, Vec<ChildRow>>,
    reg_iv: Interval,
    /// `None` until the first read, so an absent dir (mtime `None`) still loads once.
    registry: Option<(Option<SystemTime>, Vec<SessionRecord>)>,
}

impl StatusCache {
    pub(crate) fn new(every: Duration) -> Self {
        Self {
            iv: Interval::new(every),
            entries: HashMap::new(),
            runs_iv: Interval::new(every),
            runs: HashMap::new(),
            sub_iv: Interval::new(every),
            subagents: HashMap::new(),
            reg_iv: Interval::new(every),
            registry: None,
        }
    }

    /// Cached restore records (live and dormant); never touches the disk.
    pub(crate) fn records(&self) -> &[SessionRecord] {
        self.registry.as_ref().map_or(&[], |(_, r)| r.as_slice())
    }

    /// Test fixtures: cached restore records without any file.
    #[cfg(test)]
    pub(crate) fn insert_records(&mut self, records: Vec<SessionRecord>) {
        self.registry = Some((None, records));
    }

    /// Re-read the registry once per interval, only when its dir mtime moved.
    /// True when the records changed (caller rebuilds the tree: a new record may name
    /// a session tmux has not listed yet).
    pub(crate) fn refresh_registry(&mut self, now: Instant) -> bool {
        let dir = session_registry::registry_dir();
        self.refresh_registry_with(dir.as_deref(), now, session_registry::load_all)
    }

    fn refresh_registry_with(
        &mut self,
        dir: Option<&Path>,
        now: Instant,
        load: impl FnOnce() -> Vec<SessionRecord>,
    ) -> bool {
        if !self.reg_iv.due(now) {
            return false;
        }
        self.reg_iv.mark(now);
        let stamp = dir.and_then(mtime);
        if self.registry.as_ref().is_some_and(|(s, _)| *s == stamp) {
            return false;
        }
        let records = load();
        let changed = self.records() != records.as_slice();
        self.registry = Some((stamp, records));
        changed
    }

    /// Cached sub-agent rows of a session; never touches the disk.
    pub(crate) fn subagents(&self, session: &str) -> &[ChildRow] {
        self.subagents.get(session).map_or(&[], Vec::as_slice)
    }

    /// Re-read the ledgers of `sessions` once per interval (stopped rows expire by
    /// clock, so this runs even when no file moved). True when any row changed.
    pub(crate) fn refresh_subagents<'a>(
        &mut self,
        sessions: impl Iterator<Item = &'a str>,
        now: Instant,
    ) -> bool {
        self.refresh_subagents_with(sessions, now, session_subagents)
    }

    fn refresh_subagents_with<'a>(
        &mut self,
        sessions: impl Iterator<Item = &'a str>,
        now: Instant,
        mut load: impl FnMut(&str) -> Vec<ChildRow>,
    ) -> bool {
        if !self.sub_iv.due(now) {
            return false;
        }
        self.sub_iv.mark(now);
        let next: HashMap<String, Vec<ChildRow>> = sessions
            .map(|s| (s.to_string(), load(s)))
            .filter(|(_, rows)| !rows.is_empty())
            .collect();
        let changed = next != self.subagents;
        self.subagents = next;
        changed
    }

    /// Cached workers; never touches the disk.
    pub(crate) fn workers(&self, repo: &Path) -> &[OrchWorkerStatus] {
        self.entries.get(repo).map_or(&[], |(_, st)| st.workers.as_slice())
    }

    /// Run id of the cached CDAMUX agents (`None` before #178); never touches the disk.
    pub(crate) fn status_run(&self, repo: &Path) -> Option<&str> {
        self.entries.get(repo)?.1.run_id.as_deref()
    }

    /// Cached runs of `repo`; never touches the disk.
    pub(crate) fn runs(&self, repo: &Path) -> Option<&RepoRuns> {
        self.runs.get(repo).map(|(_, r)| r)
    }

    /// Test fixtures: cached workers and runs of `repo` without any file.
    #[cfg(test)]
    pub(crate) fn insert_fixture(&mut self, repo: &Path, status: OrchRunStatusFile, runs: RepoRuns) {
        self.entries.insert(repo.to_path_buf(), (None, status));
        self.runs.insert(repo.to_path_buf(), (None, runs));
    }

    /// Re-list the runs of each repo once per interval, only when `$CDAMUX_HOME/runs` moved.
    /// Returns true when any run list changed (caller redraws).
    pub(crate) fn refresh_runs<'a>(
        &mut self,
        repos: impl Iterator<Item = &'a Path>,
        now: Instant,
    ) -> bool {
        self.refresh_runs_with(repos, now, |p| RepoRuns {
            live: list_live_orch_runs(p),
            recent: list_orch_graph_runs(p, 5),
        })
    }

    fn refresh_runs_with<'a>(
        &mut self,
        repos: impl Iterator<Item = &'a Path>,
        now: Instant,
        mut load: impl FnMut(&Path) -> RepoRuns,
    ) -> bool {
        if !self.runs_iv.due(now) {
            return false;
        }
        self.runs_iv.mark(now);
        let mut next = HashMap::new();
        let mut changed = false;
        let stamp = mtime(&orch_graph_runs_dir(Path::new(".")));
        for repo in repos {
            let entry = match self.runs.remove(repo) {
                Some(old) if old.0 == stamp => old,
                old => {
                    let runs = load(repo);
                    changed |= old.is_none_or(|o| o.1 != runs);
                    (stamp, runs)
                }
            };
            next.insert(repo.to_path_buf(), entry);
        }
        changed |= !self.runs.is_empty(); // a repo left the tree
        self.runs = next;
        changed
    }

    /// Stat agents dir once per interval; parse only when mtime changed.
    /// Returns true when any worker list changed (caller redraws).
    pub(crate) fn refresh<'a>(
        &mut self,
        repos: impl Iterator<Item = &'a Path>,
        now: Instant,
    ) -> bool {
        self.refresh_with(repos, now, load_orch_run_status)
    }

    fn refresh_with<'a>(
        &mut self,
        repos: impl Iterator<Item = &'a Path>,
        now: Instant,
        mut load: impl FnMut(&Path) -> Option<OrchRunStatusFile>,
    ) -> bool {
        if !self.iv.due(now) {
            return false;
        }
        self.iv.mark(now);
        let mut next = HashMap::new();
        let mut changed = false;
        let agents_mtime = orch_agents_dir().and_then(|d| mtime(&d));
        for repo in repos {
            let entry = match self.entries.remove(repo) {
                Some(old) if old.0 == agents_mtime => old,
                old => {
                    let st = load(repo).unwrap_or_default();
                    changed |= old.is_none_or(|o| o.1 != st);
                    (agents_mtime, st)
                }
            };
            next.insert(repo.to_path_buf(), entry);
        }
        changed |= !self.entries.is_empty(); // a repo left the tree
        self.entries = next;
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn at(base: Instant, ms: u64) -> Instant {
        base + Duration::from_millis(ms)
    }

    #[test]
    fn interval_fires_once_per_period() {
        let t0 = Instant::now();
        let mut iv = Interval::new(Duration::from_millis(800));
        assert!(iv.due(t0));
        iv.mark(t0);
        assert!(!iv.due(at(t0, 799)));
        assert!(iv.due(at(t0, 800)));
    }

    #[test]
    fn no_session_row_means_zero_tmux_calls() {
        // 10 s of 100 ms ticks, non-session selection (#127): sidecar never fires.
        let t0 = Instant::now();
        let mut iv = Interval::new(Duration::from_millis(800));
        let calls = (0..100)
            .filter(|i| sidecar_step(false, false, true, &mut iv, at(t0, i * 100)) != Sidecar::Idle)
            .count();
        assert_eq!(calls, 0);

        // Same 10 s on a session row: one load, then one poll per 800 ms.
        let mut iv = Interval::new(Duration::from_millis(800));
        let calls = (0..100)
            .filter(|i| {
                sidecar_step(true, *i == 0, true, &mut iv, at(t0, i * 100)) != Sidecar::Idle
            })
            .count();
        assert_eq!(calls, 1 + 12);
    }

    #[test]
    fn agents_read_once_per_mtime_not_per_tick() {
        let dir = std::env::temp_dir().join(format!("cdamux-refresh-{}", std::process::id()));
        let agents = dir.join("agents");
        fs::create_dir_all(&agents).unwrap();
        let t0 = Instant::now();
        let mut cache = StatusCache::new(Duration::from_millis(500));
        let loads = Cell::new(0);
        let load = |_: &Path| {
            loads.set(loads.get() + 1);
            Some(OrchRunStatusFile::default())
        };
        // Inject mtime via the same stamp refresh_with reads (orch_agents_dir).
        let _guard = cdamux_core::env_lock();
        std::env::set_var("CDAMUX_HOME", &dir);
        let run = |cache: &mut StatusCache, ms: u64| {
            cache.refresh_with([dir.as_path()].into_iter(), at(t0, ms), load)
        };
        for i in 0..30 {
            run(&mut cache, i * 100);
        }
        assert_eq!(loads.get(), 1);
        let f = fs::File::open(&agents).unwrap();
        f.set_modified(SystemTime::now() + Duration::from_secs(5)).unwrap();
        for i in 30..40 {
            run(&mut cache, i * 100);
        }
        assert_eq!(loads.get(), 2);
        std::env::remove_var("CDAMUX_HOME");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn runs_relisted_only_when_the_runs_dir_moves() {
        let dir = std::env::temp_dir().join(format!("cdamux-refresh-runs-{}", std::process::id()));
        let runs_dir = dir.join("runs");
        fs::create_dir_all(&runs_dir).unwrap();
        let t0 = Instant::now();
        let mut cache = StatusCache::new(Duration::from_millis(500));
        let loads = Cell::new(0);
        let load = |_: &Path| {
            loads.set(loads.get() + 1);
            RepoRuns::default()
        };
        let _guard = cdamux_core::env_lock();
        std::env::set_var("CDAMUX_HOME", &dir);
        let repo = dir.join("repo");
        fs::create_dir_all(&repo).unwrap();
        let run = |cache: &mut StatusCache, ms: u64| {
            cache.refresh_runs_with([repo.as_path()].into_iter(), at(t0, ms), load)
        };
        assert!(run(&mut cache, 0), "first listing is a change");
        for i in 1..30 {
            assert!(!run(&mut cache, i * 100));
        }
        assert_eq!(loads.get(), 1);
        assert!(cache.runs(&repo).is_some());
        let f = fs::File::open(&runs_dir).unwrap();
        f.set_modified(SystemTime::now() + Duration::from_secs(5)).unwrap();
        for i in 30..40 {
            run(&mut cache, i * 100);
        }
        assert_eq!(loads.get(), 2);
        std::env::remove_var("CDAMUX_HOME");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn registry_reread_only_when_its_dir_moves() {
        let dir = std::env::temp_dir().join(format!("cdamux-refresh-reg-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let t0 = Instant::now();
        let mut cache = StatusCache::new(Duration::from_millis(500));
        let loads = Cell::new(0);
        let run = |cache: &mut StatusCache, ms: u64| {
            cache.refresh_registry_with(Some(&dir), at(t0, ms), || {
                loads.set(loads.get() + 1);
                Vec::new()
            })
        };
        for i in 0..30 {
            run(&mut cache, i * 100);
        }
        assert_eq!(loads.get(), 1);
        // A record written or removed moves the dir mtime: re-read once.
        let f = fs::File::open(&dir).unwrap();
        f.set_modified(SystemTime::now() + Duration::from_secs(5)).unwrap();
        for i in 30..40 {
            run(&mut cache, i * 100);
        }
        assert_eq!(loads.get(), 2);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn subagent_ledgers_read_once_per_interval_not_per_tick() {
        let t0 = Instant::now();
        let mut cache = StatusCache::new(Duration::from_millis(500));
        let loads = Cell::new(0);
        let load = |_: &str| {
            loads.set(loads.get() + 1);
            Vec::new()
        };
        // 100 ms ticks over 1 s, two sessions: one read each per 500 ms.
        for i in 0..10 {
            cache.refresh_subagents_with(["a", "b"].into_iter(), at(t0, i * 100), load);
        }
        assert_eq!(loads.get(), 4);
        assert!(cache.subagents("a").is_empty());
    }
}
