# PLAN-60 — preview sidecar + composer (v2 phase 2)

- **Status**: backlog
- **PRD**: [[../prd/PRD-41-cdamux-mission-control-tui|PRD-41]]
- **GitHub**: [#60](https://github.com/chatondearu/ai-dev-kit/issues/60)
- **Parent**: [#58](https://github.com/chatondearu/ai-dev-kit/issues/58)
- **Stack**: PR base `feat/59-cdamux-fleet-tree` (phase 1 branch)

## Summary

Split Fleet layout; Preview tab + pane tail; composer → `send_to_session`.

## Tasks

- [ ] Sidecar layout (ratatui constraints)
- [ ] `capture_pane_text` on selection (config line limit)
- [ ] Side tab stubs: Preview | Notes | Review
- [ ] Composer + Enter send; Space focuses composer
- [ ] Reuse core send helpers (no duplicate escaping)

## Verification

Manual send visible in tmux pane; `cargo test`
