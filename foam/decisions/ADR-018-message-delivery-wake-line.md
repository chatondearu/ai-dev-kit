# ADR-018 — Message delivery: wake line and hook hand-over, never a pasted body

- **Status**: accepted
- **Date**: 2026-11-23
- **Tags**: #architecture #cdamux #security
- **Amends**: [[ADR-011-cdamux-unified-state-model|ADR-011]] §4 (delivery)

## Context

`cdamux msg send` pasted a message body, with Enter, into a worker pane when the worker
looked at rest, and typed `C-u` + `📨 N messages` into an idle orchestrator. The
2026-11-23 orchestration audit (C1, H3, M5, M6, M8) found:

- **Lost wakeups**: only `answer` lifted a worker's sticky `blocked`; a `relay`, or an
  answer arriving inside the 10 s quiet window, was never pasted and never retried
  (a worker stalled 2 h 43 min, issue #402 / PR #426).
- **Injection**: a body starting with `/` or `!` runs as a Claude Code command.
- **Safety**: a sticky status hides a permission dialog; a paste's Enter answers it.
- `C-u` wipes a human draft; paste ack ordering lost or repeated lines.

Owner decisions Q1=A, Q21, Q22 (grill 2026-11-23). A local experiment (Claude Code
2.1.296) confirmed that a `Stop` hook printing `{"decision":"block","reason":…}` keeps
the turn going and the model acts on the reason; the re-fired `Stop` carries
`stop_hook_active: true`.

## Decision

1. **No body is ever typed into a pane.** Bodies reach the agent only through hooks.
2. **Turn state is a hook-written field**, `turn` on the agent record, never sticky:
   `running` at prompt submit and permission requests, `ended` at a `Stop` that ends
   the turn. It, not the display status, says whether a pane is safe to type into.
3. **Mid-turn**: Claude's `Stop` hook reads and acks the unread lines (`cdamux msg read
   --ack`) and blocks the stop with the full bodies as the reason.
4. **At rest** (`turn: ended`): after ≥ 60 s of pane quiet (`CDAMUX_WAKE_QUIET_SECS`),
   cdamux types only `📨 N messages` + Enter, no `C-u`. Claude's `UserPromptSubmit`
   hook injects the bodies and acks them. A pane not quiet yet gets a detached
   `cdamux msg wake <id>` (one per agent), which also polls mid-turn agents as a
   backstop for a line appended after the `Stop` hook read the inbox.
5. **Hookless clients** (Cursor, codex, opencode, or no turn hooks): wake line
   `📨 N messages — cdamux msg read --ack`, only when status is `idle` / unset and the
   pane tail is not a prompt; a sticky status without turn hooks waits in the inbox.
6. **Status**: `answer` or `relay` to a blocked agent lifts it to `idle` and answers its
   ask; a blocked agent's own `done` replaces `blocked` (`error` keeps it: orch.sh
   reports blocked workers as `error`).
7. Message ids are `m-` + 6 base36 chars of the second + 5 random chars; text with
   control characters other than `\n` / `\t` is refused.

## Consequences

- A permission dialog never receives a keystroke from cdamux on a hooked agent.
- A human draft is never wiped; one left untouched for 60 s is submitted with the wake
  line appended.
- An interrupted Claude turn fires no `Stop`, so `turn` stays `running`: messages wait
  for the next prompt (safe, the human is there).
- `say` (human prompt to an orchestrator) is unchanged: it is the human's own prompt.
- Tests: unit truth tables (`wake_plan`, status effects), `hooks/test.sh`, and
  `cdamux-cli/tests/delivery_tmux.rs` (real binary, hooks and tmux idle time).
