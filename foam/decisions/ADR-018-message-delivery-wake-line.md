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
   the turn (and at a `resume` / `clear` session start), `exited` at session end. For
   Claude Code it, not the display status, says whether a pane is safe to type into; a
   Claude agent with a status but no `turn` yet (startup dialogs) waits.
3. **Mid-turn**: Claude's `Stop` hook reads and acks the unread lines (`cdamux msg read
   --ack --max-bytes 8000`, acking only what it returns) and blocks the stop with the
   full bodies as the reason. Other clients firing the same events (codex) keep the
   unacked digest.
4. **At rest** (`turn: ended`): after ≥ 60 s of pane quiet (`CDAMUX_WAKE_QUIET_SECS`),
   cdamux types only `📨 N messages` + Enter, no `C-u`. Claude's `UserPromptSubmit`
   hook injects the bodies and acks them. A pane not quiet yet gets a detached
   `cdamux msg wake <id>` (one per agent), which also polls mid-turn agents as a
   backstop for a line appended after the `Stop` hook read the inbox.
5. **Other clients** (Cursor, codex, opencode, or Claude without hooks): `turn` is not
   trusted (Cursor runs queued prompts without a prompt-submit hook). Wake line
   `📨 N messages — cdamux msg read --ack`, only when status is `idle` / unset and the
   pane tail is not a prompt; a sticky status waits in the inbox. This is best effort: a
   Cursor approval dialog that does not look like a prompt and stays static for 60 s
   could take the line (follow-up: wire Cursor's pre-execution hooks to `turn`).
6. **Status**: `answer` or `relay` to a blocked agent lifts it to `idle` and answers its
   ask; a blocked agent's own `done` replaces `blocked` (`error` keeps it: orch.sh
   reports blocked workers as `error`).
7. Message ids are `m-` + 6 base36 chars of the second + 5 random chars; text with
   control characters other than `\n` / `\t` is refused.

## Consequences

- A permission dialog never receives a keystroke from cdamux on a hooked agent.
- A human draft is never wiped; one left untouched for 60 s is submitted with the wake
  line appended. Skipping the wake on a non-empty input line is not possible reliably:
  Claude Code's grey prompt suggestion reads as typed text in a pane capture (#402).
- An interrupted Claude turn fires no `Stop`, so `turn` stays `running`: messages wait
  for the next prompt (safe, the human is there); a resume or `/clear` resets it.
- A `Stop` blocks at most 3 times in a row; past that the turn ends and the rest waits for
  the wake line (a fast peer cannot keep an agent running).
- Hooks act only for the pane's top-level CLI: a nested `claude -p` inherits the agent
  id and would otherwise take the parent's inbox and flip its turn mid-turn.
- `say` (human prompt to an orchestrator) is unchanged: it is the human's own prompt.
- Tests: unit truth tables (`wake_plan`, status effects), `hooks/test.sh`, and
  `cdamux-cli/tests/delivery_tmux.rs` (real binary, hooks and tmux idle time).

## Amendment 2026-11-25 — live Cursor after `kind=done` (#444)

Keep §1 (no body in a pane) and [[ADR-019-worker-permission-boundary|ADR-019]]
(no Enter on a permission dialog / `needs_input` / Trust).

The 2026-11-23 point 5 treated every non-Claude sticky status as `WakePlan::Never`.
After `kind=done`, a live Cursor pane was reported `inbox`; the orchestrator then
pasted a body with `cdamux send` (hit on `cursor-trust` / PR #442 rebase). Point 6
lifted only `blocked`.

- **`wake_plan` (non-Claude):** sticky `done` / `error` on a **live** pane is at rest
  like `idle`. `blocked` stays inbox until `relay` / `answer`. `working` /
  `needs_input` / `pane_input` / `turn: running` still hold the wake.
- **Turn trusted when present:** if `turn` is `Some` on a non-Claude agent (Cursor
  hooks already write it), use the same branch as Claude so a permission dialog with
  `turn: running` is never woken. Slice 2 owns hook wiring; this amendment owns
  `wake_plan` only.
- **Status:** `answer` or `relay` to a live `done` / `error` agent lifts it to `idle`
  (same as `blocked`).
- **Delivery word:** `msg send` reports `inbox` only when there is no live pane or no
  safe moment (`blocked` without `turn`, `turn: exited`). A live host whose plan is
  Now / After / TurnEnd is `woken` / `scheduled` / `turn-end` — a quiet wait is not
  `inbox`.
