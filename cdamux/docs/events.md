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
| `type` | `msg`, `status`, `lifecycle`, `reply`, `human`. Ignore unknown types. |
| `ts` | Unix seconds. |

| `type` | Fields | Written when |
| ------ | ------ | ------------ |
| `msg` | `id, from, to, kind, text, ref?` (the inbox line) | `cdamux msg send` appends to an **orchestrator's** inbox (role `orchestrator`); worker inboxes are not mirrored |
| `status` | `agent, status, prev, role?, parent?` | Any agent write changes the value — `state append`/`set` (hooks, `msg send` side effects), finalize, dismiss, kill, reap; `prev` is `null` for the first one. Emitted under the agent record lock, so journal order matches write order. |
| `lifecycle` | `agent, event, role?, parent?, host?` | `event`: `spawn` (record created by a spawn), `finalize` (sweep, dismiss or kill turned the agent `error`; at most one per agent), `dismiss` (Fleet / nav `d`), `kill` (Fleet / nav `x`, `worker kill`), `archive` (record moved to `archive/` by gc, dismiss or `migrate-home`). The matching `status` item comes first. |
| `reply` | `agent, text, turn_id, truncated?` | An orchestrator's turn ends (`Stop` hook): the assistant **text blocks** of the last turn joined by a blank line — no thinking, tool calls or tool results |
| `human` | `id, agent, from, text` | `cdamux say` accepted a human prompt for orchestrator `agent` (`id` = the `say` message id), whether it was pasted or queued — see [`say.md`](say.md) |

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
- `--follow` keeps reading (500 ms poll on the journal size) until killed.
- Reading never changes any inbox ack cursor (`msg read --ack`) or agent state.
- Errors, exit 1, nothing printed (no silent skip):
  - cursor older than the oldest retained item: `cursor N expired … replay with --since 0`;
  - cursor ahead of the journal (journal deleted / `CDAMUX_HOME` reset):
    `cursor N is ahead of the journal …`.
  A client resynchronises with `--since 0`.

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
and the transcript is missing or has no assistant text, the reply text is
`tool-only turn`. `generation_id`, when the client sends one, is the `turn_id`.
A turn already recorded (same `turn_id`, kept as
`last_reply_turn` in the agent record) is not written twice. The hook allows 1s;
on timeout it retries once without `transcript_path` so the journal still gets a line.

| Client | `reply` |
| ------ | ------- |
| Claude Code | Yes: `Stop` hook + transcript. |
| Cursor | `stop` hook. Prose when the transcript matches; otherwise one `tool-only turn` line. |
| opencode, Codex | Not yet (no turn-end hook wired to `events reply`). |

Gaps: a turn whose prompt sits more than 512 KiB back in the transcript keeps only
the assistant text in that tail. If Claude continues a turn after `Stop` (a hook
blocked it), the continuation yields a second `reply` holding the turn's full text.
