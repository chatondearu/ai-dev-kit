# PLAN-41 — cdamux session state & CLI polish

- **Status**: in review
- **Issue**: [#41](https://github.com/chatondearu/ai-dev-kit/issues/41)
- **Epic**: [#40](https://github.com/chatondearu/ai-dev-kit/issues/40)
- **PRD**: [[../prd/PRD-39-cdamux|PRD-39]]

## Tasks

- [x] Pane heuristics → `SessionState` (`idle` / `active` / `needs_input`) in `cdamux-core`
- [x] `cdamux list` uses daemon socket state when available
- [x] `cdamux doctor`: verify agent binaries on PATH; optional `tailscale` presence
- [x] `attach`: `exclusive` → `attach-session -d` when not nested
- [x] `spawn`: handle duplicate `c-<basename>` (document behavior)
- [x] `spawn -i`: honor `--agent` when only path is missing (and vice versa)

## Verify

```bash
cd cdamux && cargo build && cargo test
```

## Out of scope

- Ratatui dashboard (#42)
- Tailnet HTTP (#44)
