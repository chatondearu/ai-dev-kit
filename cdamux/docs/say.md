# Human prompts: `cdamux say` and `cdamux msg answer`

Two local CLI commands for a chat bridge (or any human front end) running on the same
machine. No listener, no auth: callers decide who may run them.

`msg send --to <orchestrator>` is a mailbox (an idle orchestrator only gets a
"N messages" nudge, never the text) and `cdamux send` pastes even mid-turn. `say` is the
safe human-to-orchestrator path.

## `cdamux say <orch-id|session> --from <label> [--json] "text"`

```bash
cdamux say a-xxxxx --from alice "what is the state of the PR?"   # → m-ab12c delivered
cdamux say c-orch  --from alice --json "stop after this task"
# {"v":1,"id":"m-ab12c","target":"a-xxxxx","outcome":"queued"}
printf %s "$TEXT" | cdamux say a-xxxxx --from alice --json -     # text on stdin
```

- **Text on stdin**: with `-` as the only text argument, or no text argument at all
  (stdin not a terminal), the text is read from stdin and trailing newlines are dropped.
  Bridges should use it so human text never sits on argv (visible in `ps`). Same rules
  as argv text (empty, control characters, 2 KB → exit 2). Same for `msg answer`.

- The target (agent id, or tmux / registry session name) must be an **orchestrator**.
  A worker → exit 2, with a message pointing to `msg send`. Unknown or ambiguous → exit 2.
- The prompt reaches the pane as `[<label>] text`, pasted with Enter. The label is one
  line of at most 64 bytes, without control characters or `[` / `]` (no forged prefix). The text may hold
  `\n` and `\t`; any other control character (`\r`, ESC, …) or more than 2 KB → exit 2.
- **At rest** — reported status `idle`, a sticky `done` / `error` (a turn end does not
  lift those to `idle`), or none (no hooks), and a pane quiet for ≥ 10 s —
  → pasted at once, outcome `delivered`. Right after a turn end the pane still counts
  as active, so `say` waits out the rest of the 10 s once (the call blocks up to 10 s).
- **Anything else** — mid-turn (`working`), a permission prompt (`needs_input`: the
  paste's Enter would answer it), sticky `blocked`, no pane — → queued, outcome `queued`.
- The queue is `$CDAMUX_HOME/say/<orch-id>.jsonl`: persistent, FIFO. The next turn end
  of that orchestrator pastes **one** queued prompt; the paste starts a turn, so the
  following prompt waits for the one after. Nothing is pasted during a running turn:
  the idle check runs again under the queue lock right before the paste, and a paste
  marks a hooked orchestrator `working` under that lock, so concurrent `say` calls
  paste one prompt and queue the rest.
- A local draft left untouched for ≥ 10 s on an idle orchestrator gets the paste
  appended, and Enter submits both (the inbox wake line does the same after 60 s, ADR-018).
- Every accepted `say` appends one `human` item to the [events feed](events.md)
  (`id, agent, from, text`), at call time, delivered or queued. The paste itself — at
  once, or at a later turn end for a queued prompt — appends a `delivered` item
  (`id, agent`, same `id`): a `human` without one is still waiting in the queue.
- Each decision (`delivered` / `queued`, then `pasted` / `not_ready` / `paste_failed`)
  is traced by id in `$CDAMUX_HOME/logs/hooks.log` ([events.md](events.md#hook-log-cdamux_homelogshookslog)).

Output: `<msg-id> delivered|queued`; `--json` prints
`{"v":1,"id","target","outcome"}` (`target`: the resolved agent id). Exit 0 on success,
2 for a caller error (target, empty / too long / control-char text, bad label), 1 for an
I/O failure. Callers should time out above 10 s (see the wait above).

### Turn-end delivery

The status hook (`hooks/status.js`) runs the hidden `cdamux msg drain <agent-id>` after
it records an orchestrator's `idle` on `Stop` / `stop`, only when its queue file is not
empty (no process otherwise). The drain checks the reported status is `idle` again and
does not wait for a quiet pane. Fail-open like the other hook steps.

| Client | Queued prompts delivered at turn end |
| ------ | ------------------------------------ |
| Claude Code | Yes: `Stop` hook. |
| Cursor, Codex | Best effort: the same adapter on `stop`; a Cursor `stop` that queues a followup keeps the turn running and does not drain. |
| opencode | **Not yet**: its plugin does not call the drain. A prompt queued for an opencode orchestrator stays queued until a later `say` finds it idle. |

A prompt is popped only after a successful paste: a drain killed mid-way may paste it
twice, never lose it.

## `cdamux msg answer <ask-msg-id> --from <label> [--json] "text"`

Answers a worker's pending `ask` on a human's behalf (the ask id appears in
`cdamux events` as the `msg` item of kind `ask`).

- Answers **that one ask**: the worker gets an `answer` message `[<label>] text` from the
  agent the ask went to, with `ref` = the ask id (pasted when idle), and goes from
  `blocked` back to `idle`. Another pending ask of the same worker stays pending.
- An ask counts as answered by an `answer` whose `ref` is its id (`m-…`), whoever it
  went to. An `answer` with no `ref` or a non-id `ref` (a `--ref` path; the TUI /
  tailnet reply) still answers every earlier ask from that worker to its sender.
- The orchestrator that received the ask gets a `relay` message in its inbox
  (`from` = `human`, `ref` = the ask id, text `[<label>] answered <worker>'s ask <id>: …`),
  so it stops waiting on that ask. If only this copy fails, the answer still stands
  (exit 0), a warning goes to stderr and `--json` says `"relayed": false`: the caller
  should tell the orchestrator itself.
- Unknown id, a message that is not an `ask`, or an ask already answered → exit 2.
  The check and the answer run under one lock: of two concurrent answers, one wins.
- Output: `<answer-msg-id> answered`; `--json` prints
  `{"v":1,"id","ask","worker","outcome":"answered","relayed":true|false}`.
- Text: argv, or stdin with `-` / no text argument (see `say` above).
- Messages are capped at 2 KB per inbox line, so a long answer fails with exit 2.
