# Where verify / orch tests publish into a parent inbox (research #274)

Snapshot of `main` at `26c8e57` (2026-11-13). Read-only survey for explore map
[#273](https://github.com/chatondearu/ai-dev-kit/issues/273) / ticket
[#274](https://github.com/chatondearu/ai-dev-kit/issues/274). **No product decision** —
facts only.

Dogfood symptom (epic #265): verify stubs flooded a live parent inbox with messages
referencing PR #55, issue #1, and `/tmp/nix-shell…` paths — indistinguishable from real
`done` / `error` / `report` traffic.

Sources: [[ADR-011]](../foam/decisions/ADR-011-cdamux-unified-state-model.md) §4–§5;
`scripts/verify.sh`; `skills/core/cda-dev/scripts/lib/cdamux_state.sh`;
`skills/core/cda-dev/scripts/tests/helpers.sh`; orch smoke / pr-fix / progress tests;
`cdamux msg` (`msg.rs` / CLI).

---

## 1. Mechanism (how a test message reaches a live parent inbox)

### Publish chain

1. `orch_progress_set` → `orch_progress_write_json` → `orch_cdamux_publish_worker`
   (`progress.sh` → `cdamux_state.sh`).
2. On terminal phases `done` | `blocked` | `error`,
   `orch_cdamux_publish_worker` always calls `orch_cdamux_parent_msg_worker`.
3. `orch_graph_finish` always calls `orch_cdamux_parent_msg_run_end` (even when graph
   write is off).
4. Both helpers call `orch_cdamux_msg_send` → real CLI `cdamux msg send …` (fail-open).

### Env that selects the **live** store and recipient

| Variable | Role |
| -------- | ---- |
| `CDAMUX_HOME` | State root (`inbox/`, `agents/`, `runs/`). If unset → `${XDG_STATE_HOME:-$HOME/.local/state}/cdamux`. |
| `CDAMUX_AGENT_ID` | Treated as **parent** id: run/`worker.parent`, and `--to` for parent msgs. |
| `PATH` + `command -v cdamux` | Real binary required; helpers no-op if missing (`orch_cdamux_available`). |
| `ORCH_RUN_ID` | Required for parent msgs; mint markers under `.orch/run/`. |

`orch_cdamux_parent_msg_worker` / `_run_end` (symbols in `cdamux_state.sh`):

- `parent="${CDAMUX_AGENT_ID:-}"` — empty → return 0 (no send).
- Worker terminal → `cdamux msg send --to "$parent" --kind done|error --from "$(worker_agent_id)" [--ref worker-result.json] "worker #<id> <phase>: <msg>"`.
- Run end → `--kind report --from "$parent" --ref …/integrator-report.md "run $ORCH_RUN_ID finished: $status"`.

Wire format and orch→parent nudge behaviour match production (ADR-011 §4–§5): same
kinds, same text shape, optional `ref`. Orchestrator role → `OrchNudge` (pane
`📨 N messages — cdamux msg read`), which explains nudge spam during verify.

### Why tests look like epic #265 traffic

| Dogfood clue | Matching harness |
| ------------ | ---------------- |
| issue `#1` | `smoke_orch_dry.sh` ready-pickup `--issue 1` (and `test_verify_gate.sh`) |
| PR `#55` | `test_pr_fix_feedback.sh` `run_pr_fix 55` (red CI → agent → `orch_graph_finish`) |
| `/tmp/nix-shell…` refs | `mktemp` under nix `TMPDIR` (`/tmp/nix-shell.*`); parent_msg `--ref` points at worktree `.orch/worker-result.json` when present |

---

## 2. Pollution vs isolation conditions

### Pollutes the **real** parent inbox when **all** hold

1. Process inherits a **live** `CDAMUX_AGENT_ID` (orchestrator session / dogfood orch pane).
2. `CDAMUX_HOME` resolves to the **live** state root (inherited, or default via `HOME` /
   `XDG_STATE_HOME` — not overridden to a temp dir).
3. Real `cdamux` is on `PATH` (true under `nix develop -c bash scripts/verify.sh`).
4. Code path hits `orch_cdamux_parent_msg_worker` (phase `done`/`blocked`/`error`, not
   held `"waiting on #…"`) and/or `orch_cdamux_parent_msg_run_end`.

Then lines append to `$CDAMUX_HOME/inbox/<live-parent-id>.jsonl` and may nudge the
parent pane. Concurrently, `orch_cdamux_publish_run_json` /
`orch_cdamux_publish_worker` also write **runs/** and **agents/** under that same live
home (Fleet noise), even for non-terminal phases.

### Stays isolated when **any** of these hold

| Guard | Effect |
| ----- | ------ |
| `CDAMUX_AGENT_ID` unset | Parent msg helpers no-op; run/`worker.parent` → `null`. |
| `CDAMUX_HOME` = temp dir | Inbox/state writes stay under the temp root (even if agent id is a real-looking string). |
| No `cdamux` on `PATH` | `orch_cdamux_available` → all publish/msg no-ops. |
| Stub `cdamux` + temp `CDAMUX_HOME` (`orch_test_stub_cdamux`) | Writes only stub inbox under temp home. |
| Phase not terminal / held `"waiting on #…"` | No parent msg (state publish may still occur if cdamux+home set). |
| `cargo test` / `msg_cli.rs` etc. | Override `.env("CDAMUX_HOME", temp)` per process — do not use ambient home. |

### Critical gap: harness does **not** scrub `CDAMUX_*`

`skills/core/cda-dev/scripts/tests/helpers.sh` unsets all `ORCH_*` and some client
markers, and forces `ORCH_TMUX=0` / `ORCH_PS=false`. It does **not** unset
`CDAMUX_HOME` or `CDAMUX_AGENT_ID` (verified: after `source helpers.sh`, both remain).

`scripts/verify.sh` runs each test as `bash "$t"` with **no** env sandbox — full
inheritance from the caller (e.g. parent orch + `nix develop`).

---

## 3. What `verify.sh` runs (relevant subset)

`scripts/verify.sh` loops (among others):

- `skills/core/cda-dev/scripts/tests/test_*.sh`
- `skills/core/cda-dev/scripts/tests/smoke_orch_dry.sh`
- then `(cd cdamux && cargo test)` (CLI tests pin temp `CDAMUX_HOME` — not the gap)

No step sets/clears `CDAMUX_HOME` / `CDAMUX_AGENT_ID`.

---

## 4. File:symbol / script touchpoints

### Production send path

| Location | Symbol / role |
| -------- | ------------- |
| `skills/core/cda-dev/scripts/lib/cdamux_state.sh` | `orch_cdamux_home`, `orch_cdamux_publish_run_json` (embeds `CDAMUX_AGENT_ID` as `parent`), `orch_cdamux_publish_worker`, `orch_cdamux_msg_send`, `orch_cdamux_parent_msg_worker`, `orch_cdamux_parent_msg_run_end` |
| `skills/core/cda-dev/scripts/lib/progress.sh` | `orch_progress_write_json` → `orch_cdamux_publish_worker` |
| `skills/core/cda-dev/scripts/lib/graph.sh` | `orch_graph_finish` → `orch_cdamux_parent_msg_run_end` |
| `skills/core/cda-dev/scripts/orch.sh` | Many `orch_progress_set … done\|blocked` and `orch_graph_finish` in ready-pickup / pr-fix |
| `cdamux/crates/cdamux-core/src/msg.rs` | `msg_send`, `resolve_agent_id`, inbox append under `CDAMUX_HOME/inbox/` |
| `cdamux/crates/cdamux-cli/src/main.rs` | `cmd_msg_send` / `MsgCommands` |

### Verify entry + hermetic hole

| Location | Role |
| -------- | ---- |
| `scripts/verify.sh` | Invokes orch unit/smoke tests; no `CDAMUX_*` isolation |
| `skills/core/cda-dev/scripts/tests/helpers.sh` | Scrubs `ORCH_*` only; **leaves `CDAMUX_*`**; defines `orch_test_stub_cdamux` |

### High-risk callers (invoke orch / parent-msg path **without** temp `CDAMUX_HOME`)

| Script | Why risky under live `CDAMUX_AGENT_ID` + real `cdamux` |
| ------ | ----------------------------------------------------- |
| `smoke_orch_dry.sh` | Full `orch.sh` ready-pickup (issues 1, 2, 3, …); many `done`/`blocked` + `orch_graph_finish`; no `CDAMUX_HOME` / unset agent |
| `test_pr_fix_feedback.sh` | `orch.sh --mode pr-fix --pr 51…57` including **55**; graph finish on agent paths; no temp home |
| `test_pr_fix_gate.sh` | `orch.sh` pr-fix (early exits often skip graph; still inherits env) |
| `test_verify_gate.sh` | `orch.sh` ready-pickup `--issue 1` multiple scenarios |
| `test_lock.sh` | Live `orch.sh` ready-pickup `--issue 7` |
| `test_tmux.sh` | Sets `CDAMUX_AGENT_ID=a-parent`, `ORCH_RUN_ID=r1`, calls `orch_progress_set … blocked\|done` **without** temp `CDAMUX_HOME` (overwrites agent id; can still pollute live **state** root / wrong inbox if home default) |
| `test_verify_ci.sh` | `orch_progress_set … ci` (no parent msg; **can** still `state set` agents under live home) |
| `test_agents.sh` | Briefly exports `CDAMUX_AGENT_ID=a-parent` (agent-exec isolation test; no progress terminal by itself) |
| `test_graph_plan.sh` | `--dry-run` only for orch (dry-run exits before graph/progress) — low inbox risk |

### Intentionally isolated (temp `CDAMUX_HOME` ± stub)

| Script | Pattern |
| ------ | ------- |
| `test_parent_msg.sh` | `unset CDAMUX_AGENT_ID` then export fake parent; `CDAMUX_HOME=$TMP/...`; `orch_test_stub_cdamux` |
| `test_cdamux_publish.sh` | Same |
| `test_progress.sh`, `test_graph.sh`, `test_graph_runs.sh`, `test_graph_plan_limits.sh`, `test_pr_fix_graph_finish.sh` | Temp `CDAMUX_HOME` + stub |

### Cargo / hooks

| Location | Isolation |
| -------- | --------- |
| `cdamux/crates/cdamux-cli/tests/msg_cli.rs` (and siblings) | Per-test `.env("CDAMUX_HOME", temp)` |
| `cdamux/hooks/test.sh` | Own temp state dir |

---

## 5. Factual summary (answer to #274)

1. **Parent inbox writes** come from `orch_cdamux_parent_msg_worker` /
   `orch_cdamux_parent_msg_run_end` → `cdamux msg send`, keyed by ambient
   `CDAMUX_AGENT_ID` and resolved `CDAMUX_HOME`.
2. **`scripts/verify.sh` does not isolate** those variables; **`helpers.sh` does not
   unset them**.
3. **Smoke + several orch integration tests** never set a temp `CDAMUX_HOME`, so under a
   live orch env + nix `cdamux` they publish into the **real** inbox (and runs/agents).
4. Messages are **byte-format identical** to production sub-run notifications (kinds
   `done`/`error`/`report`, same text/`ref` conventions) → not filterable by shape alone.
5. Unit tests that pin temp `CDAMUX_HOME` + stub (or cargo tests with `.env`) **stay
   isolated**; pollution is an **ambient-env × non-hermetic orch harness** interaction,
   not the stub CLI itself.

---

## Note for map #273 parent

One-line: verify/orch smoke inherit live `CDAMUX_AGENT_ID`/`CDAMUX_HOME` because
`helpers.sh` / `verify.sh` never scrub them — that is the inbox flood vector.
