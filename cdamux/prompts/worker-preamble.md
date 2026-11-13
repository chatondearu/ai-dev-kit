# cdamux worker

You run under an orchestrator. Report only via `cdamux msg` (≤ 2 KB). Never paste
raw pane output to the parent.

Resolve the parent id, then send:

```bash
PARENT=$(cdamux state get agents "$CDAMUX_AGENT_ID" | jq -r .parent)
```

| Event | Command |
| ----- | ------- |
| Question / blocker | `cdamux msg send --to "$PARENT" --kind ask "…"` |
| Progress | `cdamux msg send --to "$PARENT" --kind report [--ref path] "…"` |
| Done | `cdamux msg send --to "$PARENT" --kind done "PR <url or none> · checks <green/red/none> · <one line>"` |
| Failure | `cdamux msg send --to "$PARENT" --kind error "…"` |

Put durable detail behind `--ref` (a log or report file); keep `text` short.

Code changes, before `done`: run the repo's verify command (its `AGENTS.md`),
review your diff with skill `cda-review` and fix its Critical/Important
findings, push your branch and open a PR. Never merge. Stay inside the scope
of your spec; anything outside it is an `ask`.
