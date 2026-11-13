# Msg kind contract and orchestrator nudge behaviour (research #275)

Snapshot of `main` at `26c8e57` (2026-11-13). Read-only survey for explore map
[#273](https://github.com/chatondearu/ai-dev-kit/issues/273). **No product policy**
on which kinds to filter or when to nudge — facts only.

Sources: [[ADR-011]](../foam/decisions/ADR-011-cdamux-unified-state-model.md) §4–§5;
`cdamux/crates/cdamux-core/src/msg.rs`; `cdamux/crates/cdamux-cli/src/main.rs`
(`MsgCommands`); `cdamux/hooks/status.js` + `ledger.js` (`unreadDigest`);
`skills/core/cda-dev/scripts/lib/cdamux_state.sh` (`orch_cdamux_parent_msg_*`);
tests `test_parent_msg.sh`, `msg.rs` unit tests, `cdamux/docs/architecture.md` /
`subagents.md`.

---

## 1. Inbox message `kind` values and semantics

### Allowed kinds (code = ADR-011 §4)

`KINDS` in `msg.rs` (also CLI help / `valid_kind`):

| `kind` | Validated by | Status side-effect (`apply_kind_status`) | Documented use |
| ------ | ------------ | ---------------------------------------- | -------------- |
| `report` | yes | none | Progress / generic notify; orch run-end parent msg |
| `ask` | yes | sender → `blocked` | Worker question; worker preamble |
| `done` | yes | none | Worker completion (preamble + orch parent msg) |
| `error` | yes | none | Failure; orch maps worker `blocked`/`error` phases here |
| `relay` | yes | none | Orchestrator forwards content to a worker (`cda-orchestrator`) |
| `answer` | yes | if recipient `status==blocked` → `idle` | Reply to a pending ask (`reply_to_ask` / Fleet composer) |

There is **no** validated kind named `blocked`. ADR-011 §5 says orch notifies on
worker `done` / `blocked` / `error`; the harness encodes board phase `blocked` as
inbox `kind=error` (see §4).

Default CLI kind when omitted: `report` (`MsgCommands::Send`).

### Wire format

One JSON line ≤ 2048 bytes (`MAX_MSG_BYTES`) in
`$CDAMUX_HOME/inbox/<to-agent-id>.jsonl`. Ack cursor:
`inbox/<agent-id>.cursor` (byte offset; advanced by `cdamux msg read --ack`).

`Message` struct fields today:

| Field | Type | Notes |
| ----- | ---- | ----- |
| `id` | string | `m-xxxxx` from urandom |
| `ts` | u64 | unix seconds |
| `from` | string | agent id (`--from` or `$CDAMUX_AGENT_ID`) |
| `to` | string | resolved agent id |
| `kind` | string | one of `KINDS` |
| `text` | string | required, non-empty |
| `ref` | optional string | path to `worker-result.json`, log, or integrator report |

CLI:

```text
cdamux msg send --to <id|host|name> --kind report|ask|done|error|relay|answer [--ref path] [--from id] "text"
cdamux msg read [--agent id] [--ack] [--digest]
```

Target resolution (`resolve_agent_id`): agent id → registry `cdamux_id` → scan
agents by `host` or logical `name`. Ambiguous bare names require
`$CDAMUX_AGENT_ID` to pick the caller's child.

---

## 2. Delivery paths: paste vs nudge vs digest

Inbox append is always the source of truth; pane delivery is best-effort
(`msg_send` comment).

### Role → delivery mode (`delivery_mode_for_role`)

| Target `agents/<id>.role` | Mode | Pane behaviour |
| ------------------------- | ---- | -------------- |
| `worker` | `WorkerPaste` | `send_to_session(host, text, enter)` — pastes **message body** |
| anything else (`orchestrator`, `agent`, …) | `OrchNudge` | optional idle nudge — does **not** paste `text` |

### Orchestrator nudge (`should_nudge` / `nudge_text`)

Constants: `DEFAULT_NUDGE_QUIET_SECS = 10`.

`should_nudge(status, idle_seconds, quiet_secs)` is true iff:

1. reported status is `SessionState::Idle` (`load_reported_status`), **and**
2. pane idle seconds ≥ `quiet_secs.max(1)` (from tmux `list_panes_all`, or test inject).

When true and host session exists: types into the pane:

```text
📨 {N} messages — cdamux msg read
```

where `N = unread_count(to).max(1)`. No daemon. Soft-fail if no live session /
send fails (`nudged=false`, message still in inbox).

Not nudged when: status missing/non-idle, quiet window not met, no host, or
target is a worker (paste path instead).

ADR-011 consequence: user typing after 10 s quiet can collide with the nudge.

### Hook digest (separate from nudge)

Opt-in `--cdamux-hooks` (`status.js` + `ledger.js` `unreadDigest`):

| Client | Event | Injection |
| ------ | ----- | --------- |
| Claude Code | `Stop` | `additionalContext` (skipped if `stop_hook_active`) |
| Claude Code | `UserPromptSubmit` | `additionalContext` |
| Cursor | `stop` | `followup_message` when `loop_count==0` |
| Cursor | `beforeSubmitPrompt` | **cannot** inject |

Digest does **not** ack. Shape matches Rust `format_unread_digest`:

```text
cdamux: {n} unread message(s) — run: cdamux msg read --ack
- [{kind}] from {from}: {text preview ≤80 chars}
… (at most 5 lines, then "…and K more")
```

CLI `--digest` prints the same Rust formatter over unread (not yet acked) lines.

ADR-011 §4 also lists: busy orch continues via turn-end digest; optional future
native wake (`msg read --wait`) — not implemented in this snapshot.

---

## 3. Fields present vs missing for noise filtering

### On the message today

Present: `id`, `ts`, `from`, `to`, `kind`, `text`, optional `ref`.

**Absent on the message** (relevant to hygiene / filtering):

| Concern | Today | Gap |
| ------- | ----- | --- |
| Run id | Not a message field. Orch run-end puts run id **inside `text`** (`run $ORCH_RUN_ID finished: …`). Agent JSON may have `run`; msg does not copy it. | No structured `run` / `run_id` on inbox lines → hard to filter by run without parsing text |
| Parent link | `from`/`to` are agent ids; agent file has `parent`. Msg has no `parent` field. | Correlation is join via agent state, not the line |
| Self-messages | Allowed. Run-end explicitly sets `from=parent` and `to=parent` (comment: harness has no separate run Agent). No `self` / `from==to` flag. | Readers must compare `from==to` themselves; nudge/digest treat them like any unread |
| Ephemeral / suppress-nudge | No field, no CLI flag | Every valid send can trigger OrchNudge if idle+quiet; no “inbox only” / “silent” marker |
| Kind granularity for board `blocked` | Phase `blocked` → msg `kind=error` | Cannot distinguish harness “blocked” vs true `error` by kind alone (phase survives only in `text`) |
| Held frontier | Orch skips send when `msg` matches `"waiting on #"*` — **sender-side**, not a message flag | Nothing on the wire marks held vs real terminal |

### Related but out-of-band

- Agent `parent` / `run` live under `$CDAMUX_HOME/agents/<id>.json`, not inbox.
- Idempotency markers: `.orch/run/<run_id>.<wid>.parent_msg` and
  `.orch/run/<run_id>.parent_msg_run` (repo artefacts), not inbox fields.
- Fleet badges: `unread_count` + `has_pending_ask` (ask without later answer) —
  no kind filter.

---

## 4. `orch.sh` parent_msg kinds and text patterns

Implementation: `skills/core/cda-dev/scripts/lib/cdamux_state.sh`.
Fail-open via `orch_cdamux_msg_send` → `cdamux msg send`. Requires
`$CDAMUX_AGENT_ID` (parent) and `$ORCH_RUN_ID`. Evidence:
`skills/core/cda-dev/scripts/tests/test_parent_msg.sh`.

### Per-worker terminal (`orch_cdamux_parent_msg_worker`)

Triggered from progress publish when phase is terminal for notify.

| Board / progress `phase` | Inbox `kind` | Skipped when |
| ------------------------ | ------------ | ------------ |
| `done` | `done` | — |
| `blocked` | `error` | `msg` matches `"waiting on #"*` (held frontier) |
| `error` | `error` | — |
| other | (no send) | — |

- **Text**: `worker #$wid $phase: $msg` (default msg `worker #$wid $phase` if empty).
- **from**: worker agent id (`orch_cdamux_worker_agent_id`).
- **to**: `$CDAMUX_AGENT_ID`.
- **ref**: worktree `.orch/worker-result.json` when present.
- **Idempotency**: marker `.orch/run/${ORCH_RUN_ID}.${wid}.parent_msg` (write after
  send; failed send may drop that attempt). New run id → notify again for same wid.

### Run end (`orch_cdamux_parent_msg_run_end`)

Called from `orch_graph_finish` / graph finish path.

| Field | Value |
| ----- | ----- |
| `kind` | `report` (always) |
| `text` | `run $ORCH_RUN_ID finished: $status` (`$status` e.g. `ok`, `blocked`) |
| `from` | parent (`$CDAMUX_AGENT_ID`) — **self-message** |
| `to` | parent |
| `ref` | `${ORCH_REPO}/.orch/integrator-report.md` |
| marker | `.orch/run/${ORCH_RUN_ID}.parent_msg_run` |

ADR-011 §5 wording: one msg per worker `done`/`blocked`/`error` and one at run
end with integrator `ref`. Implementation matches volume/triggers; kind mapping
for phase `blocked` is `error`, not a distinct kind; run-end uses `report` with
status embedded in text (including literal `blocked` in the finished status
string when the run aggregate is blocked).

---

## 5. Out of scope for this ticket

- Choosing which kinds suppress nudge or digest.
- Changing `blocked`→`error` mapping or adding new kinds.
- Product PR / merge. Hand-off: map #273 → PRD / later children.
