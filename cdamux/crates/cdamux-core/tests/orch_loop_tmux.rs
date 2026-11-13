//! End-to-end orchestration loop (PRD-315) on a throwaway tmux server + temp CDAMUX_HOME:
//! `orch new` → `worker spawn` with a spec → worker `ask` → orchestrator `answer` →
//! worker `done` → orchestrator digest. One assertion per hop, each with a named message.

use cdamux_core::{
    env_lock, format_unread_digest, inbox_badges, msg_send, read_messages, spawn_orchestrator,
    spawn_worker, state_append, state_get, AgentEntry, AgentsConfig, OrchNewRequest, StateKind,
    WorkerSpawnRequest,
};
use std::process::Command;
use std::time::{Duration, Instant};

const SOCKET: &str = "cdamux-orch-loop-test";
/// Injected pane quiet window: the freshly spawned panes are not 15 s old yet.
const QUIET: Option<Option<u32>> = Some(Some(15));

fn which(bin: &str) -> Option<String> {
    Command::new("sh")
        .args(["-c", &format!("command -v {bin}")])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn tmux(args: &[&str]) -> String {
    let out = Command::new("tmux").args(args).output().expect("tmux");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn git(repo: &str, args: &[&str]) {
    let ok = Command::new("git")
        .args(["-C", repo, "-c", "user.email=t@t", "-c", "user.name=t"])
        .args(args)
        .status()
        .unwrap()
        .success();
    assert!(ok, "git {args:?} failed");
}

fn squash(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

/// Whole pane history (the spec scrolls past 24 rows), whitespace stripped (wrap-safe).
fn wait_for_pane(session: &str, needle: &str) -> String {
    let want = squash(needle);
    let start = Instant::now();
    loop {
        let text = squash(&tmux(&["capture-pane", "-p", "-J", "-S", "-", "-t", &format!("={session}:")]));
        if text.contains(&want) || start.elapsed() > Duration::from_secs(5) {
            return text;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn status(id: &str) -> String {
    state_get(StateKind::Agents, id)
        .unwrap()
        .and_then(|v| v["status"].as_str().map(str::to_owned))
        .unwrap_or_default()
}

#[test]
fn orchestrator_loop_spawn_ask_answer_done() {
    let Some(real_tmux) = which("tmux") else {
        eprintln!("tmux not on PATH: skipped");
        return;
    };
    if which("git").is_none() {
        eprintln!("git not on PATH: skipped");
        return;
    }

    let tmp = tempfile::tempdir().unwrap();
    let bin = tmp.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let wrapper = bin.join("tmux");
    std::fs::write(
        &wrapper,
        format!("#!/bin/sh\nexec {real_tmux} -L {SOCKET} -f /dev/null \"$@\"\n"),
    )
    .unwrap();
    // Stub client: print its args (orchestrator boot prompt / worker first prompt), stay alive.
    let stub = bin.join("claude");
    std::fs::write(&stub, "#!/bin/sh\nprintf '%s\\n' \"$@\"\nexec sleep 60\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    for f in [&wrapper, &stub] {
        std::fs::set_permissions(f, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    let repo = tmp.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    let repo_s = repo.to_str().unwrap();
    git(repo_s, &["init", "-b", "main"]);
    std::fs::write(repo.join("README"), "x").unwrap();
    git(repo_s, &["add", "README"]);
    git(repo_s, &["commit", "-m", "init"]);

    let _env = env_lock();
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default());
    std::env::set_var("PATH", &path);
    std::env::set_var("TMUX_TMPDIR", tmp.path());
    std::env::set_var("CDAMUX_HOME", tmp.path().join("cdamux"));
    std::env::remove_var("TMUX");

    let agents = AgentsConfig {
        default: "claude".into(),
        claude: Some(AgentEntry { command: "claude".into(), resume: None }),
        cursor: None,
        opencode: None,
        pi: None,
    };

    // Hop 1: orch new → orchestrator agent with role + run.
    let orch = spawn_orchestrator(OrchNewRequest {
        path: &repo,
        agent: Some("claude"),
        agents: &agents,
        switch_to_session: false,
    })
    .expect("hop 1 (orch new) failed");
    let oid = orch.agent_id.as_str();
    let rec = state_get(StateKind::Agents, oid).unwrap().expect("hop 1: orchestrator record missing");
    assert_eq!(rec["role"], "orchestrator", "hop 1: wrong role");
    // The stub reports idle, as the real status hook would after the boot turn.
    state_append(StateKind::Agents, oid, &serde_json::json!({ "status": "idle" })).unwrap();

    // Hop 2: worker spawn --prompt-file → spec reaches the worker's first prompt intact.
    let spec = "Implement the `api` endpoint.\nKeep $HOME untouched; it's \"quoted\" & (nested).\nDone = tests green.";
    let prompt = tmp.path().join("spec.md");
    std::fs::write(&prompt, spec).unwrap();
    let w = spawn_worker(WorkerSpawnRequest {
        name: "api",
        caller_id: oid,
        agent: None,
        prompt_file: Some(&prompt),
        here: false,
        worktree_root: Some(&tmp.path().join("wts")),
        agents: &agents,
        cwd: &repo,
    })
    .expect("hop 2 (worker spawn) failed");
    let wid = w.agent_id.as_str();
    let wrec = state_get(StateKind::Agents, wid).unwrap().expect("hop 2: worker record missing");
    assert_eq!(wrec["parent"], oid, "hop 2: worker not parented to the orchestrator");
    assert_eq!(wrec["role"], "worker", "hop 2: wrong role");
    let pane = wait_for_pane(&w.host, spec);
    assert!(pane.contains(&squash(spec)), "hop 2: spec not intact in worker first prompt: {pane}");
    state_append(StateKind::Agents, wid, &serde_json::json!({ "status": "idle" })).unwrap();

    // Hop 3: worker ask → worker blocked, `?` + ✉1 on the orchestrator side, orch nudged.
    let ask = msg_send(wid, oid, "ask", "which-db-please", None, 15, QUIET)
        .expect("hop 3 (ask) failed");
    assert_eq!(ask.delivery_word(), "nudged", "hop 3: ask not nudged to the idle orchestrator");
    assert_eq!(status(wid), "blocked", "hop 3: worker not blocked after ask");
    assert_eq!(inbox_badges(wid), (0, true), "hop 3: worker badges want (0 unread, ? pending)");
    assert_eq!(inbox_badges(oid), (1, false), "hop 3: orchestrator badges want ✉1");
    let opane = wait_for_pane(&orch.host, "📨 1 messages — cdamux msg read --ack");
    assert!(opane.contains("cdamuxmsgread--ack"), "hop 3: nudge missing from orch pane: {opane}");

    // Hop 4: orchestrator answer → worker unblocked, `?` cleared, answer pasted in its pane.
    let ans = msg_send(oid, wid, "answer", "use-postgres", None, 15, QUIET)
        .expect("hop 4 (answer) failed");
    assert_eq!(ans.delivery_word(), "pasted", "hop 4: answer not pasted to the idle worker");
    assert_eq!(status(wid), "idle", "hop 4: worker still blocked after answer");
    assert_eq!(inbox_badges(wid), (0, false), "hop 4: ? not cleared / pasted line still unread");
    let wpane = wait_for_pane(&w.host, "use-postgres");
    assert!(wpane.contains("use-postgres"), "hop 4: answer missing from worker pane: {wpane}");

    // Hop 5: worker done → status done, orchestrator ✉2 and nudged with the count.
    let done = msg_send(wid, oid, "done", "pr-opened", None, 15, QUIET).expect("hop 5 (done) failed");
    assert_eq!(done.delivery_word(), "nudged", "hop 5: done not nudged to the orchestrator");
    assert_eq!(status(wid), "done", "hop 5: worker not done");
    assert_eq!(inbox_badges(oid), (2, false), "hop 5: orchestrator badges want ✉2");
    let opane = wait_for_pane(&orch.host, "📨 2 messages — cdamux msg read --ack");
    assert!(opane.contains("📨2messages"), "hop 5: count nudge missing from orch pane: {opane}");

    // Hop 6: orchestrator digest lists ask + done in order, then the inbox is acked.
    let msgs = read_messages(oid, true).expect("hop 6 (read) failed");
    let kinds: Vec<_> = msgs.iter().map(|m| m.kind.as_str()).collect();
    assert_eq!(kinds, ["ask", "done"], "hop 6: digest order");
    let digest = format_unread_digest(&msgs);
    assert!(digest.contains("which-db-please") && digest.contains("pr-opened"), "hop 6: digest: {digest}");
    assert_eq!(inbox_badges(oid), (0, false), "hop 6: inbox not acked after read --ack");

    let sock = tmux(&["display-message", "-p", "#{socket_path}"]);
    assert!(
        sock.contains(tmp.path().to_str().unwrap_or("___")),
        "refusing kill-server: socket {sock} not under temp"
    );
    let _ = Command::new("tmux").args(["kill-server"]).status();
}
