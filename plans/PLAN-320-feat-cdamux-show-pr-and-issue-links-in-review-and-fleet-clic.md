# PLAN-320 — feat(cdamux): show PR and issue links in Review and Fleet, clickable

- **Status**: ready
- **PRD**: [[../prd/PRD-315-cdamux-orchestration-cockpit|PRD-315-cdamux-orchestration-cockpit]]
- **GitHub**: [#320](https://github.com/chatondearu/ai-dev-kit/issues/320)
- **Blocked by**: #319
- **Date**: 2026-11-14

## Summary

Show the linked PR / issue in the Review tab header and as a Fleet badge, clickable (OSC 8) and openable with `o`.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-001-host-tools-vs-kit|ADR-001]] | Document tmux `hyperlinks` feature; do not change user config |

Do not violate accepted ADRs without superseding them.

## Scope

### In scope

- Review header `PR #N <state> · issue #M`
- Fleet row badge (compact, e.g. `#312`)
- OSC 8 hyperlinks around the labels; plain text fallback
- `o` opens PR (else issue) via `gh pr view --web` / `gh issue view --web` or the OS opener

### Out of scope

- Detection (#319)

## Technical approach

`review_tui.rs` header, `fleet_draw.rs` badge, a small OSC 8 helper (ratatui cell symbols carrying the escape, or post-draw write); keymap entry. Doc: `cdamux/docs/dogfood.md` tmux `terminal-features ',*:hyperlinks'`.

## Tasks

- [x] Header + badge + snapshots
- [x] OSC 8 helper + test of escape output
- [x] `o` action + test with stub opener
- [x] Doc tmux hyperlinks

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| OSC 8 breaks layout width | Zero-width escapes; test with ratatui buffer |

## Test plan

- [x] Snapshots with/without link; escape-sequence unit test

## Verification

- [ ] `cargo test --no-fail-fast` in `cdamux/` (known on main: `home_repair_tmux`)
- [ ] `bash scripts/verify.sh` (host PATH; `nix develop` only if a tool is missing)
- [ ] tmux tests on throwaway servers only (`cdamux/AGENTS.md`)
