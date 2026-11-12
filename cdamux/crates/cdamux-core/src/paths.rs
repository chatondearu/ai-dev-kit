//! State and config path resolution (ADR-011 §2).

use std::path::PathBuf;

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

/// `$CDAMUX_HOME/<sub>` (sessions, agents, runs, …).
pub fn cdamux_state_dir(sub: &str) -> Option<PathBuf> {
    Some(cdamux_home()?.join(sub))
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
}
