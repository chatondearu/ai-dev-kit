# Sub-agent ledger

CLI sub-agents (Claude Code `Task` agents and the like) run inside their parent
CLI process: tmux cannot see them. Each CLI gets a small hook adapter that appends
start/stop events to a per-tmux-session ledger; cdamux reads the ledger and shows
one Fleet child row per sub-agent under its session, with the sub-agent transcript
tail as preview.

This file is the contract for every adapter (#159 Claude Code, #160 the others).

## CLI support

| CLI | Adapter | Status |
| --- | ------- | ------ |
| Claude Code | `cdamux/hooks/claude-subagent.js` on `SubagentStart` / `SubagentStop` | supported |
| Cursor | — | U4 (#160) |
| opencode | — | U4 (#160) |
| pi | — | U4 (#160) |

Install (opt-in, Claude only): `./install.sh --claude --cdamux-hooks`.
`./install.sh --uninstall` removes it. A plain `./install.sh` neither adds nor
removes it.

## Path

```text
${XDG_STATE_HOME:-$HOME/.local/state}/cdamux/agents/<tmux-session>.jsonl
```

- `<tmux-session>` is the name of the session the CLI runs in, read at hook time
  with `tmux display-message -p -t "$TMUX_PANE" '#S'`. Every `/` in the name is
  replaced by `_`.
- Not inside tmux (`$TMUX` unset) → the adapter writes nothing.

## Line format

One JSON object per line, appended (`O_APPEND`), UTF-8, `\n`-terminated.

| Field | Type | Required | Meaning |
| ----- | ---- | -------- | ------- |
| `ts` | integer | yes | Unix time in seconds of the event |
| `cli` | string | yes | `claude`, `cursor`, `opencode`, `pi` |
| `event` | string | yes | `start` or `stop` |
| `id` | string | yes | Sub-agent id, identical on its `start` and `stop` lines |
| `parent_id` | string | no | Parent CLI session id (Claude: `session_id`) |
| `name` | string | no | Agent type / name shown in the row (`Explore`, `cda-review`) |
| `transcript` | string | no | Absolute path of the sub-agent transcript (JSONL) |
| `status` | string | no | On `stop`: `error` marks a failed sub-agent; anything else = done |

Unknown fields are ignored, so adapters may add their own.

Example:

```json
{"ts":1793721846,"cli":"claude","event":"start","id":"a719bf5550ead5628","parent_id":"f3cb1c9b-8fa2-4438-b65b-ba365b6994c4","name":"Explore","transcript":"/Users/me/.claude/projects/-src-app/f3cb1c9b-8fa2-4438-b65b-ba365b6994c4/subagents/agent-a719bf5550ead5628.jsonl"}
{"ts":1793721990,"cli":"claude","event":"stop","id":"a719bf5550ead5628","parent_id":"f3cb1c9b-8fa2-4438-b65b-ba365b6994c4","name":"Explore","transcript":"/Users/me/.claude/projects/-src-app/f3cb1c9b-8fa2-4438-b65b-ba365b6994c4/subagents/agent-a719bf5550ead5628.jsonl"}
```

## Trimming

After each append the adapter keeps the last **500** lines of the file
(rewrite to a temp file, then rename).

## How cdamux reads it

- Re-parsed only when the file mtime or length changes.
- Lines are folded by `id` in first-`start` order; malformed lines and a `stop`
  without its `start` (trimmed away) are skipped.
- A sub-agent is **running** until its `stop`, then **done** (or **error**) and kept
  for **10 minutes** after the stop `ts`.
- Preview: the last messages of `transcript` (bounded read of its last 256 KiB):
  user/assistant text only; tool calls, tool results and thinking are skipped.
  The transcript parser expects Claude-style lines
  (`{"message":{"role":…,"content":"…" | [{"type":"text","text":…}]}}`); an adapter
  for a CLI with another transcript format either leaves `transcript` out or
  extends `transcript_tail` in `cdamux-core`.
- Waiting-only mode hides sub-agent rows.

## Adapter rules

- Fail-open: any error exits 0 without output; never block or slow the CLI.
- Under 50 ms per event, local file only, no network.
- Skip events that are not real sub-agents (Claude: an empty `agent_type` is one
  of Claude's internal agents, such as prompt suggestions).

## Claude Code notes

From the hook docs (`SubagentStart`: `agent_id`, `agent_type`; `SubagentStop`:
also `agent_transcript_path`, `last_assistant_message`):

- `SubagentStart` has no `agent_transcript_path`; the adapter derives
  `<dir of transcript_path>/<session_id>/subagents/agent-<id>.jsonl`, the layout
  Claude Code writes on disk.
- The docs show `agent_id` both with and without an `agent-` prefix; the adapter
  strips it so `start` and `stop` share one `id`.
- A leading `~/` in a path is expanded to `$HOME`.
