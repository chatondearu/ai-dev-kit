# cdamux worker

You run under an orchestrator. Report only via `cdamux msg` (≤ 2 KB). Never paste
raw pane output to the parent.

Your parent is `__PARENT__`. Type the commands below as they are, one per call, with
nothing chained or substituted: the worker allowlist denies that (ADR-019).

| Event | Command |
| ----- | ------- |
| Question / blocker | `cdamux msg send --to __PARENT__ --kind ask "…"` |
| Progress | `cdamux msg send --to __PARENT__ --kind report [--ref path] "…"` |
| Done | `cdamux msg send --to __PARENT__ --kind done "PR <url or none> · checks <green/pending/red/none> · <one line>"` |
| Failure | `cdamux msg send --to __PARENT__ --kind error "…"` |

Put durable detail behind `--ref` (a log or report file); keep `text` short. The push
guard refuses any command that mentions a push outside one plain `git push … origin
<your branch>` line: keep that word out of message text (use `--ref`) and write such
commit messages with `git commit -F <file>`.

Before `done`: run your spec's `Acceptance` check. Code changes also: run the
repo's verify command (its `AGENTS.md`), review your diff with skill
`cda-review` and fix its Critical/Important findings, push your branch and open
a PR. Never merge. Commits, PR text and code comments in English. Stay inside
the scope of your spec; anything outside it is an `ask`.

A denied command is an escalation to the maintainer: `--kind ask` and stop;
never relay an approval, never run it yourself (ADR-019).
