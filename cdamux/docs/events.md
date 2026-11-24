# Events feed (`cdamux events`, schema `v: 1`)

A read-only, append-only feed to **follow orchestrators** from outside cdamux
(e.g. a chat bridge): worker messages to an orchestrator, agent status changes,
and the orchestrator's own replies to the human — prose only, no tool detail.
Backing store: the `$CDAMUX_HOME/events.jsonl` journal (ADR-011 §2). Local CLI
only: no network listener, no access control (callers enforce who may read).

```bash
cdamux events --since 0 --json                 # replay every retained item
cdamux events --follow --since <cursor> --json # replay after cursor, then stream
cdamux events --agent a-xxxxx --json           # one orchestrator (and its workers)
cdamux events --orchestrators --json           # msg / reply / human + orchestrator status
```

Without `--json` each item prints as one short human line
(`<cursor> <type> <who> <first text line>`). `--since` defaults to `0`.

## Items

One JSON object per line. Every item has:

| Field | Meaning |
| ----- | ------- |
| `v` | Schema version, `1`. New optional fields may appear without a bump; a removed or retyped field bumps it. |
| `cursor` | Opaque string; pass it back as `--since`. Today a sequence number that keeps counting across rotation. |
| `type` | `msg`, `status`, `lifecycle`, `reply`, `human`, `delivered`. Ignore unknown types. |
| `ts` | Unix seconds. |

| `type` | Fields | Written when |
| ------ | ------ | ------------ |
| `msg` | `id, from, to, kind, text, ref?` (the inbox line) | `cdamux msg send` appends to an **orchestrator's** inbox (role `orchestrator`); worker inboxes are not mirrored |
| `status` | `agent, status, prev, role?, parent?, turn_id?, tool_only?` | Any agent write changes the value — `state append`/`set` (hooks, `msg send` side effects), finalize, dismiss, kill, reap; `prev` is `null` for the first one. Emitted under the agent record lock, so journal order matches write order. Also an orchestrator turn that ended with **no assistant prose**: `tool_only: true`, its `turn_id`, and `status` = `prev` = the recorded status (no change) — see [Client coverage](#client-coverage-for-reply). |
| `lifecycle` | `agent, event, role?, parent?, host?` | `event`: `spawn` (record created by a spawn), `finalize` (sweep, dismiss or kill turned the agent `error`; at most one per agent), `dismiss` (Fleet / nav `d`), `kill` (Fleet / nav `x`, `worker kill`), `archive` (record moved to `archive/` by gc, dismiss or `migrate-home`). The matching `status` item comes first. |
| `reply` | `agent, text, turn_id, truncated?` | An orchestrator's turn ends (`Stop` hook) with assistant prose: the **text blocks** of the last turn joined by a blank line — no thinking, tool calls or tool results. Never written for a turn without prose. |
| `human` | `id, agent, from, text` | `cdamux say` accepted a human prompt for orchestrator `agent` (`id` = the `say` message id), whether it was pasted or queued — see [`say.md`](say.md) |
| `delivered` | `id, agent` | That `human` prompt (same `id`) was pasted into orchestrator `agent`'s pane: at once by `say`, or later by the turn-end drain of a queued prompt. At most one per `id` unless a killed drain pasted it twice. Always after its `human` item. Best effort (fail-open journal write): a `human` without a `delivered` is *probably* still queued — the [hook log](#hook-log-cdamux_homelogshookslog) (`pasted` by id) is authoritative. |

Changes since the first `v: 1` (additive, no bump): `delivered` items; `status` items with
`tool_only` / `turn_id` (a tool-only turn used to be a `reply` with text `tool-only turn`);
exit code 3 for a refused cursor; `events --follow` reads incrementally.

Example:

```json
{"v":1,"cursor":"42","type":"msg","ts":1795000000,"id":"m-ab12c","from":"a-w1","to":"a-or1","kind":"done","text":"PR #12 · checks green"}
{"v":1,"cursor":"43","type":"status","ts":1795000001,"agent":"a-w1","status":"done","prev":"working","role":"worker","parent":"a-or1"}
{"v":1,"cursor":"44","type":"reply","ts":1795000030,"agent":"a-or1","text":"Worker w1 opened PR #12; checks are green.","turn_id":"5f1c…"}
```

### Filters

- `--agent <id>`: items whose `agent`, `to`, `from` or `parent` is `<id>` — an
  orchestrator's replies, its inbox traffic and its workers' status.
- `--orchestrators`: every `msg` / `reply` / `human` item, and `status` /
  `lifecycle` items of agents whose role is `orchestrator`.

## Cursor semantics

- `--since <c>` prints items with a cursor **strictly after** `c`; `--since 0`
  prints every retained item. Re-running with the last printed cursor prints nothing
  until a new item lands.
- `--follow` keeps reading until killed: one full read, then every 500 ms only the
  lines appended to the live segment (one `open` + `fstat` when nothing changed). A
  rotation or a replaced journal falls back to a full read from the last printed cursor.
- Reading never changes any inbox ack cursor (`msg read --ack`) or agent state.
- **Refused cursor — exit 3**, stderr starting with `events: cursor refused: ` (stable
  prefix; pinned by `tests/events_cli.rs`). Nothing is printed for a refusal at start;
  under `--follow` the items already printed stay valid. Two reasons, after the prefix:
  - `cursor N expired: oldest retained item is M; replay with --since 0` — older than
    the oldest retained item;
  - `cursor N is ahead of the journal (last item L); was it reset? …` — journal
    deleted or `CDAMUX_HOME` reset, also when it happens while following.
  A client resynchronises with `--since 0`. Only exit 3 means "drop my cursor".
- Exit 2: malformed `--since`. Exit 1: any other failure (I/O on `$CDAMUX_HOME`, …) —
  retry with the same cursor; never treat it as a refusal.

## Retention and caps

- Two segments: `events.jsonl` (live) rotates to `events.1.jsonl` once it reaches
  2 MiB, replacing the previous one — so 2–4 MiB of history is retained.
- `msg.text` follows the inbox cap (line ≤ 2 KB). `reply.text` is capped at 8 KiB
  (cut on a character boundary) with `truncated: true`.
- Writers are fail-open: a journal error never fails `msg send`, a state write or a
  hook; that item is lost (a `reply` turn is then left unrecorded and retried at the
  next turn end). A line torn by a failed write is skipped by readers; the next item
  starts on its own line.

## Client coverage for `reply`

The status hook (`hooks/status.js`) calls `cdamux events reply <agent-id>` with the
hook payload on stdin at turn end, only for agents whose role is `orchestrator`.
cdamux reads `transcript_path` (last 512 KiB), finds the last user line holding text,
and keeps the assistant text after it. The transcript can lag the `Stop` hook: when
the payload's `last_assistant_message` is not the end of that text (or there is no
transcript), the reply is `last_assistant_message` alone. When that message is empty
and the transcript is missing or has no assistant text, the turn is a **tool-only
turn**: one `status` item with `tool_only: true` (no `reply`, no text), written under
the agent record lock. `generation_id`, when the client sends one, is the `turn_id`.
A turn already recorded (same `turn_id`, kept as
`last_reply_turn` in the agent record) is not written twice. The hook allows 1s;
on timeout it retries once without `transcript_path` so the journal still gets a line.

| Client | `reply` |
| ------ | ------- |
| Claude Code | Yes: `Stop` hook + transcript. |
| Cursor | `stop` hook. Prose when the transcript matches; otherwise one tool-only `status`. |
| opencode, Codex | Not yet (no turn-end hook wired to `events reply`). |

Gaps: a turn whose prompt sits more than 512 KiB back in the transcript keeps only
the assistant text in that tail. If Claude continues a turn after `Stop` (a hook
blocked it), the continuation yields a second `reply` holding the turn's full text.

## Hook log (`$CDAMUX_HOME/logs/hooks.log`)

Not part of the feed: a local debug trace of every delivery / hand-over decision, so a
prompt, message or reply that never showed up leaves a reason. One JSON object per line,
`{ts, src, agent, decision, …}`, with ids, kinds and counts only — **never message
text**. File `0600` in a `0700` dir; past 256 KiB it moves to `hooks.log.1` (replacing
it), so at most ~512 KiB. Writers are fail-open. No compatibility promise on the
decisions; for humans and `jq`, not for clients.

| `src` | `decision` (extra fields) | Writer |
| ----- | ------------------------- | ------ |
| `say` | `delivered` / `queued` (`id, status, idle, host`) | `cdamux say` |
| `say` | `pasted` / `not_ready` / `paste_failed` (`id`) | paste of a queued prompt (`say`, turn-end drain) |
| `answer` | `answered` / `relay_failed` (`id, ask, worker`) | `cdamux msg answer` |
| `msg` | `woken` / `scheduled` / `turn-end` / `inbox` (`id, kind, from`; `agent` = recipient) | `cdamux msg send` |
| `reply` | `reply` / `tool_only` / `duplicate` / `unwritten` (`turn_id`) | `cdamux events reply` (turn end) |
| `hook` | `stop_handover` (`ids`), `stop_block_cap` / `handover_ack_failed` (`unread`), `prompt_handover` (`event, ids`), `digest` (`event`), `followup_digest`, `reply_retried_without_transcript`, `reply_failed` (`exit, timeout?`), `drain_failed` (`timeout`) | `hooks/status.js` / `ledger.js` |

```bash
jq -c 'select(.agent=="a-xxxxx")' "$CDAMUX_HOME/logs/hooks.log" | tail
jq -c 'select(.id=="m-…")' "$CDAMUX_HOME/logs/hooks.log"   # where did my prompt go?
```
