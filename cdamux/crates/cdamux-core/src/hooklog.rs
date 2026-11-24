//! Production hook log `$CDAMUX_HOME/logs/hooks.log`: one JSON line per delivery /
//! hand-over decision (`say` pasted or queued, inbox delivery, turn-end reply, Stop
//! hand-over), so a prompt or reply that never showed up leaves a trace. Ids, kinds and
//! counts only, never message text. 0600, bounded, fail-open. Format: `docs/events.md`.
//! `hooks/ledger.js` writes the same lines.

use crate::paths::{cdamux_state_dir, create_private_dir, private_file_options};
use serde_json::{json, Value};
use std::io::{self, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Past this size the log moves to `hooks.log.1` (replacing it): at most ~2× on disk.
pub const HOOK_LOG_MAX_BYTES: u64 = 256 * 1024;

/// Append `{ts, src, agent, decision, ...extra}`; never fails the caller.
pub fn hook_log(src: &str, agent: &str, decision: &str, extra: Value) {
    let Some(dir) = cdamux_state_dir("logs") else {
        return;
    };
    let mut line = json!({
        "ts": SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs()),
        "src": src, "agent": agent, "decision": decision,
    });
    if let (Some(m), Value::Object(extra)) = (line.as_object_mut(), extra) {
        m.extend(extra);
    }
    let _ = create_private_dir(&dir)
        .and_then(|()| append_bounded(&dir.join("hooks.log"), &line, HOOK_LOG_MAX_BYTES));
}

fn append_bounded(path: &Path, line: &Value, max: u64) -> io::Result<()> {
    if std::fs::metadata(path).is_ok_and(|m| m.len() >= max) {
        // shortcut: two writers rotating at once may drop a line; fine for a debug log.
        std::fs::rename(path, path.with_extension("log.1"))?;
    }
    let mut f = private_file_options().append(true).open(path)?;
    f.write_all(format!("{line}\n").as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::env_lock;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn private_bounded_and_text_free() {
        let _g = env_lock();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("CDAMUX_HOME", dir.path());
        hook_log("say", "a-o", "queued", json!({ "id": "m-1" }));
        std::env::remove_var("CDAMUX_HOME");
        let path = dir.path().join("logs/hooks.log");
        let v: Value = serde_json::from_str(std::fs::read_to_string(&path).unwrap().trim()).unwrap();
        assert_eq!((v["src"].as_str(), v["agent"].as_str()), (Some("say"), Some("a-o")));
        assert_eq!((v["decision"].as_str(), v["id"].as_str()), (Some("queued"), Some("m-1")));
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&path), 0o600);
        assert_eq!(mode(&dir.path().join("logs")), 0o700);

        // Bounded: past `max` the file rotates to `.1`, so at most two files.
        for i in 0..20 {
            append_bounded(&path, &json!({ "i": i }), 64).unwrap();
        }
        assert!(std::fs::metadata(&path).unwrap().len() < 64 + 16);
        assert_eq!(mode(&path.with_extension("log.1")), 0o600);
        let names = std::fs::read_dir(dir.path().join("logs")).unwrap().count();
        assert_eq!(names, 2);
    }
}
