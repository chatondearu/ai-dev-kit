# Foam graph

Foam's **wikilink graph** is the durable knowledge layer (ADRs, PRDs, features).
VS Code Foam visualizes it in the editor; `../index.md` is the human Map of Content.

Validate an intra-issue graph plan before `orch.sh --graph-plan`:

```bash
bash ~/.cursor/skills/cda-graph/scripts/graph-plan-validate.sh plans/graph/issue-42.json
```

Operator guide: [doc/graph-orchestration.md](../../doc/graph-orchestration.md).

## Runtime orchestration graph

During `orch.sh`, the harness writes **only** `.orch/graph/current.json` (gitignored under `.orch/`).
cdamux reads that file for the dynamic Fleet side panel.

See [[../decisions/ADR-005-cda-graph-orchestration|ADR-005]].
