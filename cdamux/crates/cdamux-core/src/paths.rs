//! State and config path resolution (ADR-011 §2).

use std::fs::OpenOptions;
use std::io;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

/// `$CDAMUX_HOME`, else `${XDG_STATE_HOME:-~/.local/state}/cdamux`.
/// `None` when neither `CDAMUX_HOME`, `XDG_STATE_HOME`, nor `HOME` can resolve a base.
pub fn cdamux_home() -> Option<PathBuf> {
    if let Some(home) = std::env::var_os("CDAMUX_HOME").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(home));
    }
    let base = match std::env::var_os("XDG_STATE_HOME").filter(|v| !v.is_empty()) {
        Some(x) => PathBuf::from(x),
        None => PathBuf::from(std::env::var_os("HOME")?).join(".local/state"),
    };
    Some(base.join("cdamux"))
}

/// `$CDAMUX_HOME` when the process has it set (non-empty). Only an explicit home is
/// propagated into spawned panes: the XDG fallback is what #391 is leaving behind.
pub fn explicit_cdamux_home() -> Option<PathBuf> {
    std::env::var_os("CDAMUX_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// `$CDAMUX_HOME/<sub>` (sessions, agents, runs, …).
pub fn cdamux_state_dir(sub: &str) -> Option<PathBuf> {
    Some(cdamux_home()?.join(sub))
}

/// Create `dir` (and parents) 0700, and tighten `$CDAMUX_HOME` to 0700 when `dir` is under
/// it: briefs, inboxes and events carry message text other local users must not read (L4).
pub fn create_private_dir(dir: &Path) -> io::Result<()> {
    std::fs::DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
    // Best effort, once: a home the user does not own must not break every state write.
    if let Some(home) = cdamux_home().filter(|h| dir.starts_with(h)) {
        if std::fs::metadata(&home).is_ok_and(|m| m.permissions().mode() & 0o077 != 0) {
            let _ = std::fs::set_permissions(home, std::fs::Permissions::from_mode(0o700));
        }
    }
    Ok(())
}

/// `create(true)` options whose new file is 0600 (existing files keep their mode).
pub fn private_file_options() -> OpenOptions {
    let mut o = OpenOptions::new();
    o.create(true).mode(0o600);
    o
}

/// Serialise tests (unit + integration) that mutate process env (`CDAMUX_HOME`, `PATH`, …).
pub fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cdamux_home_prefers_env_then_xdg() {
        let _g = env_lock();
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        std::env::set_var("CDAMUX_HOME", &home);
        assert_eq!(cdamux_home().as_deref(), Some(home.as_path()));
        assert_eq!(
            cdamux_state_dir("sessions").as_deref(),
            Some(home.join("sessions").as_path())
        );
        std::env::remove_var("CDAMUX_HOME");

        let xdg = dir.path().join("xdg");
        std::env::set_var("XDG_STATE_HOME", &xdg);
        assert_eq!(cdamux_home().unwrap(), xdg.join("cdamux"));
        std::env::remove_var("XDG_STATE_HOME");
    }

    #[test]
    fn private_dir_and_file_are_owner_only() {
        let _g = env_lock();
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        std::fs::create_dir(&home).unwrap();
        std::fs::set_permissions(&home, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::env::set_var("CDAMUX_HOME", &home);
        create_private_dir(&home.join("inbox")).unwrap();
        let f = home.join("inbox/a.jsonl");
        private_file_options().append(true).open(&f).unwrap();
        std::env::remove_var("CDAMUX_HOME");
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&home), 0o700, "pre-existing home tightened");
        assert_eq!(mode(&home.join("inbox")), 0o700);
        assert_eq!(mode(&f), 0o600);
    }
}
