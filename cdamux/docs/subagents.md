# Sub-agent ledger

CLI sub-agents (Claude Code `Task` agents and the like) run inside their parent
CLI process: tmux cannot see them. Each CLI gets a small hook adapter that appends
start/stop events to a per-tmux-session ledger; cdamux reads the ledger and shows
one Fleet child row per sub-agent under its session, with the sub-agent transcript
tail as preview.

This file is the contract for every adapter (#159 Claude Code, #160 the others).
All adapters share one writer, `cdamux/hooks/ledger.js` (tmux session lookup,
append, trim).

## CLI support

| CLI | Adapter | Status |
| --- | ------- | ------ |
| Claude Code | `cdamux/hooks/claude-subagent.js` on `SubagentStart` / `SubagentStop` | supported |
| Cursor | `cdamux/hooks/cursor-subagent.js` on `subagentStart` / `subagentStop` | supported (see notes: payloads from docs, not recorded live) |
| opencode | `cdamux/hooks/opencode-subagent.js` plugin on `session.created` / `session.idle` / `session.error` / `session.deleted` | supported, no transcript preview (payloads from SDK types, not run live) |
| pi | none | gap: pi has no sub-agents (see notes); revisit if pi or a pi package adds a sub-agent/child-session event |

Install (opt-in): `./install.sh --cdamux-hooks` wires every enabled tool (Claude
`settings.json`, Cursor `hooks.json`, opencode plugin link in
`~/.config/opencode/plugins/`), including the session id capture below.
`./install.sh --uninstall` removes them. A plain `./install.sh` neither adds nor
removes them.

## Session id capture (restore registry)

The same opt-in hooks keep `agent_session_id` current in the restore record
`${XDG_STATE_HOME:-$HOME/.local/state}/cdamux/sessions/<tmux-session>.json`
([ADR-009](../../foam/decisions/ADR-009-cdamux-session-restore-registry.md)), so a
restore resumes the conversation that was live, not the one the session started
with (`/clear`, `/resume`, a new chat). Writer: `setAgentSessionId` in `ledger.js`.

| CLI | Event | Id field | Status |
| --- | ----- | -------- | ------ |
| Claude Code | `SessionStart` (`startup`, `resume`, `clear`, `compact`) → `cdamux/hooks/session-start.js` | `session_id` | supported; payload from the hook docs, not recorded live |
| Cursor | `sessionStart` → `cdamux/hooks/session-start.js` | `session_id` (docs: "same as `conversation_id`"), else `conversation_id` | docs only: not run live, and the docs do not say whether `cursor-agent` (CLI) fires it |
| opencode | plugin `session.created` with no `parentID` (main session) → `opencode-subagent.js` | `info.id` | SDK types only, not run live; resuming with `--session` fires no `session.created`, the recorded id is already right |
| pi | none | — | gap: no adapter (as for sub-agents) |

Rules (on top of the adapter rules below):

- Only an existing record is updated (sessions cdamux spawned); never creates one.
- Only `agent_session_id` and `updated_ts` change; every other key is kept. Written
  atomically (tmp file + rename); a record that is not a JSON object is left as is.
- Claude: the hook prints nothing (its `SessionStart` stdout would become context).
  Its entry sits next to the kiss `SessionStart` entry in `settings.json`; each
  installer only strips entries under its own hooks dir.
- Cursor: prints `{}` (`sessionStart` is fire-and-forget, not a permission hook).
- Only the pane's top-level CLI writes. A nested CLI started from the pane (a
  `claude -p` or a headless orch worker run from a tool call, which inherit
  `TMUX_PANE`) fires its own `SessionStart` and would point restore at a worker.
  Rule: walk the parent chain (`ps -o ppid=,comm= -p <pid>`, at most 8 hops) from the
  hook's parent (opencode: the plugin's own process) up to the pane's
  `#{pane_pid}`; accept only if at most one process on the way is not a shell
  (`sh`, `bash`, `zsh`, `fish`, … — pane, login and `sh -c` hook shells). The
  top-level CLI is that one; a nested CLI also passes the top CLI, so it sees two.
  `ps` failure or the pane out of reach → no write. Checked live on the
  maintainer's tree (`fish -c fish` pane → fish → claude): top-level shape writes,
  nested shape does not, ~15 ms. Not an env var: `CLAUDECODE=1` (set in Claude's
  tool shells) reaches every nested CLI, and whether the top CLI's own hooks see it
  is undocumented; the parent chain works the same for every CLI.
- shortcut: a launcher that is not a shell and stays alive (npm's node wrapper of
  opencode) counts as a second process, so that agent's id is not captured.
- Without the hooks, Claude still resumes from the id cdamux pre-assigns at spawn
  (`--session-id`), but a `/clear` then resumes the pre-clear conversation.

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

## Cursor notes

Docs: <https://cursor.com/docs/hooks> (`subagentStart`: `subagent_id`,
`subagent_type`, `parent_conversation_id`; `subagentStop`: `subagent_type`,
`status` = `completed` | `error` | `aborted`, `agent_transcript_path`; no id).

- `subagentStop` has no id and `subagentStart` has no transcript path. The
  adapter links them by file name: Cursor writes
  `<project>/agent-transcripts/<parent>/subagents/<subagent_id>.jsonl` (seen on
  disk), so `start` derives that path from the common `transcript_path`, and `stop`
  takes its `id` from the basename of `agent_transcript_path`. A `stop` without
  `agent_transcript_path` is skipped (the row then stays running until trimmed).
- `subagentStart` is a permission hook: empty or invalid output blocks the
  sub-agent, so the adapter always prints `{"permission":"allow"}` (`{}` on stop).
- Only `status: error` marks a failed row; `aborted` shows as done.
- Cursor transcripts put `role` at the top level (`{"role":…,"message":{"content":[…]}}`);
  `transcript_tail` reads both shapes.
- Not checked live: the docs do not say whether `cursor-agent` (CLI) runs
  `subagentStart`/`subagentStop` (only `workspaceOpen` is stated for the CLI), and
  the payloads in the tests are shaped from the docs. Verify with a real
  sub-agent run before relying on it.

## opencode notes

Docs: plugins <https://opencode.ai/docs/plugins/> (`event` hook, plugins in
`~/.config/opencode/plugins/`); event and `Session` types (`parentID?`, `title`)
in the SDK, `packages/sdk/js/src/gen/types.gen.ts`
(<https://opencode.ai/docs/sdk/>).

- A child session (created by the `task` tool) is a `Session` with `parentID`.
  `start` on `session.created` with `parentID`; `stop` on the child's
  `session.idle` / `session.deleted`, or with `status: error` on `session.error`.
- No `transcript`: opencode does not keep a Claude-style JSONL transcript, so the
  row has no preview.
- shortcut: a child resumed after its `session.idle` is not shown again.
- Not run live (opencode is not installed on the maintainer's machine); tests
  feed sample events shaped from the SDK types.
- The existing `kiss.mjs` is linked into the singular `plugin/` dir; the docs name
  `plugins/` (`.js`/`.ts`), which is where this plugin goes.

## pi notes

Docs: <https://github.com/badlogic/pi-mono/blob/main/packages/coding-agent/README.md>
("Pi ships with powerful defaults but skips features like sub-agents and plan
mode") and <https://github.com/badlogic/pi-mono/blob/main/packages/coding-agent/docs/extensions.md>
(extension events: `session_start`, `agent_start`/`agent_end`, `tool_call`,
`tool_execution_start`/`end`, …).

pi has no sub-agent concept, so no extension event marks a child agent starting or
stopping (`parentToolCallId` and `ctx.newSession({ parentSession })` exist but are
not sub-agents). Agents a user spawns from pi are separate processes, which cdamux
already sees as tmux panes. No adapter shipped.
