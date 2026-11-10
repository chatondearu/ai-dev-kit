# Client capabilities matrix for orchestrator and workers

Research ticket: [#230](https://github.com/chatondearu/ai-dev-kit/issues/230), part of map
[#228](https://github.com/chatondearu/ai-dev-kit/issues/228). Date: 2026-11-10.

Question: which capabilities do Claude Code, Cursor CLI (`cursor-agent` / `agent`), opencode
and codex CLI share, so that **any** of them can run the orchestrator session and receive
worker reports?

Versions checked locally: `claude` 2.1.296, `cursor-agent` 2026.10.01-e373342. opencode and
codex are not installed on this machine: docs only.

## Matrix

Legend: **yes** = documented and/or seen locally; **partial** = exists with a limit; **no** =
not documented. Citations are numbered; see [Sources](#sources).

| Capability | Claude Code | Cursor CLI | opencode | codex CLI |
| --- | --- | --- | --- | --- |
| Session start event | yes `SessionStart` [C1][L1] | yes `sessionStart` [U1]; CLI firing: community reports only [U4] | yes `session.created` event [O1][L3] | yes `SessionStart` [X1] |
| User prompt submit | yes `UserPromptSubmit` [C1][L2] | yes `beforeSubmitPrompt` [U1][L4] | yes `prompt` session hook (V2) [O2]; `chat.message` (V1) | yes `UserPromptSubmit` [X1] |
| Stop / turn end | yes `Stop` [C1] | yes `stop` [U1]; fires in CLI per [U4] (`afterAgentResponse` does not) | yes `session.idle` [O1] | yes `Stop` [X1]; legacy `notify` = `agent-turn-complete` [X4] |
| Notification / needs input | yes `Notification` [C1] | **no** (no such event) [U1] | partial `permission.asked` [O1] | partial `PermissionRequest` hook [X1]; `tui.notifications` `approval-requested` [X4] |
| Sub-agent start / stop | yes `SubagentStart`/`SubagentStop` [C1][L1] | yes `subagentStart`/`subagentStop` [U1][L5]; CLI not verified [L6] | partial: child sessions (`parentID`) via `session.created`/`session.idle` [L3] | yes `SubagentStart`/`SubagentStop` [X1] |
| Hook injects text into next model turn | yes `additionalContext` on SessionStart, UserPromptSubmit, Stop ("conversation continues") [C1][L2] | partial: `sessionStart.additional_context`; `stop.followup_message` auto-submits next user message; **not** on `beforeSubmitPrompt` [U1] | yes `context` hook adds system text per model call; `ctx.session.prompt`/`synthetic` sends a message [O2][L7] | yes `additionalContext` on SessionStart/UserPromptSubmit; `Stop` `decision:block` `reason` becomes a new prompt [X1] |
| Background command wake (idle session resumes when a job exits) | yes: `run_in_background` Bash, `Monitor` ("Claude interjects when an event arrives"), `asyncRewake` hooks [C2][C1] | **no** (not documented) [U2] | partial: no built-in; a plugin can call `ctx.session.prompt` from any event [O2] | **no**: background terminals (`/ps`) do not wake the agent [X2][X5] |
| Interactive mode + initial prompt argument | yes `claude [prompt]` [L8] | yes `agent [prompt...]` [L9][U2] | partial: `opencode --prompt "…"` flag (positional = project dir) [O3] | yes `codex [PROMPT]` [X2] |
| MCP client | yes `claude mcp`, `--mcp-config` [L8] | yes `mcp.json`, `agent mcp` [U3][L9] | yes `mcp` local/remote [O4] | yes `codex mcp`, `~/.codex/config.toml` [X2] |
| Resume session by id (interactive) | yes `claude --resume <id>` [L8] | yes `agent --resume <chatId>`, `agent ls` [L9][U2] | yes `opencode --session <id>` / `-s` [O3] | yes `codex resume <SESSION_ID>` / `--last` [X2] |

## Common to all four

1. **Session start event** (context injection at start: Claude, Cursor, codex natively; opencode
   via its `context` hook).
2. **Turn-end event that can continue the turn with a message**: Claude `Stop`
   (block/`additionalContext`), Cursor `stop.followup_message`, codex `Stop` `decision:block`,
   opencode `session.idle` + `ctx.session.prompt`. This is the one *push-into-the-model* point
   every client has.
3. **User prompt submit event** (observe; inject everywhere except Cursor).
4. **MCP client.**
5. **Resume by id** and **start interactive with an initial prompt** (opencode through
   `--prompt`, the three others positionally) — enough for a spawner to launch and restore
   orchestrator and worker sessions with a per-client argv table.

**Not common:**

- **Idle wake.** Only Claude Code (background Bash, `Monitor`, `asyncRewake`) and opencode (a
  plugin can post a prompt at any time) can start a turn on their own. Cursor CLI and codex
  have no documented way for an external event to wake an idle session.
- **Notification / needs-input event** (Cursor has none; opencode and codex only for
  permissions).
- **Sub-agent events** are native on three; opencode infers them from child sessions.

## Minimum mechanism for any client as orchestrator

Workers already run in tmux (ADR-002, ADR-007) and the map locks "shared state files + a small
CLI" (#228). Given the matrix, the smallest mechanism that works for **every** client is:

1. **Write path (no client feature needed):** a worker reports with the CLI
   (`<cli> msg send --to <orchestrator> …`), which appends one JSON line to the orchestrator's
   inbox under the env-overridable state root. Any client can run a shell command, so every
   worker can report.
2. **Read path (no client feature needed):** the orchestrator reads with the same CLI
   (`<cli> msg read`), told to do so by its skill/prompt.
3. **Wake (the only missing common piece):** when a line lands and the orchestrator pane is
   idle, **cdamux types a one-line nudge into the pane** (`tmux send-keys "inbox: 1 new report —
   run <cli> msg read" Enter`). An interactive TUI takes typed input whatever the client, so this
   is the single mechanism that wakes Cursor CLI and codex too. It needs the idle signal cdamux
   already computes for Fleet (PR #220) and must not fire while the user is typing.

Optional per-client upgrades, same inbox:

- **Turn-end delivery (all four):** a turn-end hook that, when the inbox is non-empty, continues
  the turn with the reports (Claude `Stop`, Cursor `stop.followup_message`, codex `Stop` block,
  opencode `session.idle` → `session.prompt`). Avoids the typed nudge when a turn is running.
- **Native wake:** Claude `Monitor` / background `tail` on the inbox; opencode plugin
  `ctx.session.prompt` on an inbox change.
- **MCP** `inbox_read` / `send` tools are possible on all four but are pull-only: an MCP server
  cannot start a turn, so MCP does not replace the wake step.

## Local findings worth a follow-up

- `skills/quality/cda-kiss/hooks/lib.js` `writeHookOutput` returns `additional_context` for
  Cursor `beforeSubmitPrompt`; Cursor documents only `continue` / `user_message` there [U1], so
  the per-prompt KISS reminder is likely dropped on Cursor (session-start injection still works).
- Cursor CLI hook firing is still unverified live here (ADR-007 "Not verified live"); a live
  probe with a project `.cursor/hooks.json` was not run in this research.

## Sources

Official docs:

- [C1] Claude Code hooks — https://code.claude.com/docs/en/hooks (event list; "Add context for
  Claude"; `asyncRewake`: "runs in the background and wakes Claude on exit code 2").
- [C2] Claude Code tools reference — https://code.claude.com/docs/en/tools-reference (Background
  commands; Monitor: "You keep working in the same session and Claude interjects when an event
  arrives"); interactive mode — https://code.claude.com/docs/en/interactive-mode#background-bash-commands
- [U1] Cursor hooks — https://cursor.com/docs/agent/hooks (`sessionStart` `additional_context`;
  `stop` `followup_message`: "Cursor will automatically submit it as the next user message";
  `beforeSubmitPrompt` outputs `continue`, `user_message`; no notification event).
- [U2] Cursor CLI overview — https://cursor.com/docs/cli/overview (`agent "…"` initial prompt,
  `agent --resume="chat-id"`, `agent ls`).
- [U3] Cursor CLI MCP — https://cursor.com/docs/cli/mcp ("The Cursor CLI supports Model Context
  Protocol (MCP) servers").
- [U4] Cursor forum (community, not official) —
  https://forum.cursor.com/t/cursor-cli-doesnt-send-all-events-defined-in-hooks/148316 and
  https://forum.cursor.com/t/hooks-afteragentresponse-afteragentthought-not-firing-in-headless-cli/156220
- [O1] opencode plugins (V1) — https://opencode.ai/docs/en/plugins/ (events `session.created`,
  `session.idle`, `permission.asked`, `tui.prompt.append`, …).
- [O2] opencode server plugins (V2) — https://opencode.ai/docs/build/plugins/server (`context`
  hook; `prompt` hook; `ctx.session.prompt` / `synthetic` / `command`).
- [O3] opencode CLI `tui` flags — `services/site/src/docs/v1/content/cli.mdx` in
  https://github.com/anomalyco/opencode (`--continue`, `--session`, `--prompt`).
- [O4] opencode MCP — https://opencode.ai/docs/mcp-servers/
- [X1] Codex hooks — https://learn.chatgpt.com/docs/hooks ("Hooks are enabled by default";
  Stop: "automatically creates a new continuation prompt that acts as a new user prompt").
- [X2] Codex CLI reference — https://learn.chatgpt.com/docs/cli/reference (`PROMPT`,
  `codex resume`, `codex mcp`; no background-wake feature documented).
- [X4] Codex config reference — https://learn.chatgpt.com/docs/config-file/config-reference.md
  (`notify`, `tui.notifications`).
- [X5] Codex background terminal (third-party) —
  https://codex.danielvaughan.com/2026/03/28/codex-background-terminal-dev-server-workflows/

Local evidence (this repo / this machine):

- [L1] `cdamux/hooks/session-start.js`, `cdamux/hooks/claude-subagent.js`.
- [L2] `install.sh` `wire_claude_hooks` (SessionStart/SubagentStart/UserPromptSubmit) and
  `skills/quality/cda-kiss/hooks/lib.js` `writeHookOutput`.
- [L3] `cdamux/hooks/opencode-subagent.js` (child sessions via `session.created` / `session.idle`).
- [L4] `install.sh` `wire_cursor_hooks`; `~/.cursor/hooks.json` (read-only: `sessionStart`,
  `beforeSubmitPrompt`, `stop`, `afterAgentResponse`, `subagentStart`/`Stop`, `sessionEnd`).
- [L5] `cdamux/hooks/cursor-subagent.js`.
- [L6] `cdamux/docs/subagents.md`, `foam/decisions/ADR-007-cdamux-session-hierarchy.md`
  ("Not verified live: Cursor CLI hooks, opencode events").
- [L7] `skills/quality/cda-kiss/opencode/kiss.mjs` (`ctx.session.hook('context')`,
  `ctx.session.prompt`).
- [L8] `claude --help` 2.1.296: `Usage: claude [options] [command] [prompt]`,
  `-r, --resume [value]`, `--mcp-config`, `mcp`, `--bg`.
- [L9] `cursor-agent --help` 2026.10.01: `Usage: agent [options] [command] [prompt...]`,
  `--resume [chatId]`, `mcp`, `ls`, `resume`.
- `skills/core/cda-dev/scripts/lib/agents.sh` `_orch_agent_exec`: today's workers run print
  mode only (`claude -p`, `cursor-agent -p`, `opencode run`, `codex exec`).
