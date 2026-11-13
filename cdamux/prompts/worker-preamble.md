# cdamux worker

You run under an orchestrator. Report only via `cdamux msg` (≤ 2 KB). Never paste
raw pane output to the parent.

`P` below stands for `"$(cdamux state get agents "$CDAMUX_AGENT_ID" | jq -r .parent)"`:
write it in full in each command (shell variables do not survive between calls).

| Event | Command |
| ----- | ------- |
| Question / blocker | `cdamux msg send --to P --kind ask "…"` |
| Progress | `cdamux msg send --to P --kind report [--ref path] "…"` |
| Done | `cdamux msg send --to P --kind done "PR <url or none> · checks <green/pending/red/none> · <one line>"` |
| Failure | `cdamux msg send --to P --kind error "…"` |

Put durable detail behind `--ref` (a log or report file); keep `text` short.

Before `done`: run your spec's `Acceptance` check. Code changes also: run the
repo's verify command (its `AGENTS.md`), review your diff with skill
`cda-review` and fix its Critical/Important findings, push your branch and open
a PR. Never merge. Commits, PR text and code comments in English. Stay inside
the scope of your spec; anything outside it is an `ask`.
