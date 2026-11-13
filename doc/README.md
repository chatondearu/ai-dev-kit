# ai-dev-kit documentation

English operator docs for this kit. Skills and agent prompts live under
`skills/`; this tree explains how to run them.

| Page | Contents |
| ---- | -------- |
| [Architecture](architecture.md) | Headless vs orch vs orch-auto; fast-lane; data flow |
| [Orchestration](orchestration.md) | Commands: `orch.sh`, `orch-auto.sh`, `headless.sh`, systemd |
| [Graph orchestration](graph-orchestration.md) | `cda-graph`, `$CDAMUX_HOME/runs/`, graph-plan, cdamux view |
| [Orca ↔ kit mapping](orca-mapping.md) | Competitor crosswalk; visibility quick wins status |
| [Configuration](configuration.md) | Env vars, locks, logs, host CLIs, optional Mise Node/Rust pins, `cda-floorplan` output paths |

Related (not duplicated here):

- Ideology: [`PHILOSOPHY.md`](../PHILOSOPHY.md)
- ADRs: [`foam/decisions/`](../foam/decisions/)
- External tools: [`docs/recommended.md`](../docs/recommended.md) ([`docs/README.md`](../docs/README.md))
- Doc layout (agent memory vs operator docs): [`foam/product/documentation-layout.md`](../foam/product/documentation-layout.md)
- Skills index: [`skills/README.md`](../skills/README.md)
