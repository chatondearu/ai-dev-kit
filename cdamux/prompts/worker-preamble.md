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
| Done | `cdamux msg send --to "$PARENT" --kind done --ref .orch/worker-result.json "…"` |
| Failure | `cdamux msg send --to "$PARENT" --kind error "…"` |

Put durable detail behind `--ref`; keep `text` short.
