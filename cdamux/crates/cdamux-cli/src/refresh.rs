//! Refresh scheduling for the dashboard loop: one interval per source and an
//! mtime-gated cache for `status.json`, so the 100 ms input poll never does I/O.
//! Every method takes `now` so tests drive a fake clock.

use cdamux_core::{load_orch_run_status, orch_run_status_path, OrchWorkerStatus};
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

/// Orch workers per repo, re-read only when the file's mtime moved.
pub(crate) struct StatusCache {
    iv: Interval,
    entries: HashMap<PathBuf, (Option<SystemTime>, Vec<OrchWorkerStatus>)>,
}

impl StatusCache {
    pub(crate) fn new(every: Duration) -> Self {
        Self {
            iv: Interval::new(every),
            entries: HashMap::new(),
        }
    }

    /// Cached workers; never touches the disk.
    pub(crate) fn workers(&self, repo: &Path) -> &[OrchWorkerStatus] {
        self.entries.get(repo).map_or(&[], |(_, w)| w.as_slice())
    }

    /// Stat each repo once per interval; parse only when mtime changed.
    /// Returns true when any worker list changed (caller redraws).
    pub(crate) fn refresh<'a>(
        &mut self,
        repos: impl Iterator<Item = &'a Path>,
        now: Instant,
    ) -> bool {
        self.refresh_with(repos, now, |p| load_orch_run_status(p).map(|s| s.workers))
    }

    fn refresh_with<'a>(
        &mut self,
        repos: impl Iterator<Item = &'a Path>,
        now: Instant,
        mut load: impl FnMut(&Path) -> Option<Vec<OrchWorkerStatus>>,
    ) -> bool {
        if !self.iv.due(now) {
            return false;
        }
        self.iv.mark(now);
        let mut next = HashMap::new();
        let mut changed = false;
        for repo in repos {
            let mtime = fs::metadata(orch_run_status_path(repo))
                .and_then(|m| m.modified())
                .ok();
            let entry = match self.entries.remove(repo) {
                Some(old) if old.0 == mtime => old,
                old => {
                    let workers = load(repo).unwrap_or_default();
                    changed |= old.is_none_or(|o| o.1 != workers);
                    (mtime, workers)
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
    fn status_json_read_once_per_mtime_not_per_tick() {
        // No tempfile dev-dep in this crate: a pid-scoped dir under the OS temp dir.
        let dir = std::env::temp_dir().join(format!("cdamux-refresh-{}", std::process::id()));
        let path = orch_run_status_path(&dir);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "{}").unwrap();
        let t0 = Instant::now();
        let mut cache = StatusCache::new(Duration::from_millis(500));
        let loads = Cell::new(0);
        let load = |_: &Path| {
            loads.set(loads.get() + 1);
            Some(Vec::new())
        };
        let run = |cache: &mut StatusCache, ms: u64| {
            cache.refresh_with([dir.as_path()].into_iter(), at(t0, ms), load)
        };
        // 100 ms ticks over 3 s with an unchanged file: parsed once.
        for i in 0..30 {
            run(&mut cache, i * 100);
        }
        assert_eq!(loads.get(), 1);
        // mtime moves: next due check re-parses exactly once.
        let f = fs::OpenOptions::new().write(true).open(&path).unwrap();
        f.set_modified(SystemTime::now() + Duration::from_secs(5)).unwrap();
        for i in 30..40 {
            run(&mut cache, i * 100);
        }
        assert_eq!(loads.get(), 2);
        let _ = fs::remove_dir_all(&dir);
    }
}
