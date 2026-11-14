# PLAN-318 — feat(cdamux): Foam tab toggle between raw markdown and rendered preview

- **Status**: ready
- **PRD**: [[../prd/PRD-315-cdamux-orchestration-cockpit|PRD-315-cdamux-orchestration-cockpit]]
- **GitHub**: [#318](https://github.com/chatondearu/ai-dev-kit/issues/318)
- **Blocked by**: #317
- **Date**: 2026-11-14

## Summary

`m` toggles the Foam preview between raw markdown and the rendered view; the mode persists while browsing.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-012-foam-browser-replaces-notes-sidecar|ADR-012]] | Read-only preview |

Do not violate accepted ADRs without superseding them.

## Scope

### In scope

- Raw mode: file source, same caps and scroll as rendered
- `m` toggle + hint in footer/help; state kept across files

### Out of scope

- Syntax highlighting of markdown source

## Technical approach

`foam.rs`: raw read path sharing `read_capped`; UI state flag in the Fleet sidecar state; draw raw lines in `fleet_draw.rs`; key in `keymap.rs`.

## Tasks

- [x] Raw preview API + test (caps)
- [x] Toggle key + state + hint
- [x] Snapshots for both modes

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Key clash with existing `m` binding | Check `keymap.rs`; pick a free key and document |

## Test plan

- [x] Snapshot raw vs rendered; toggle keeps mode on file change

## Verification

- [ ] `cargo test --no-fail-fast` in `cdamux/` (known on main: `home_repair_tmux`)
- [ ] `bash scripts/verify.sh` (host PATH; `nix develop` only if a tool is missing)
- [ ] tmux tests on throwaway servers only (`cdamux/AGENTS.md`)
