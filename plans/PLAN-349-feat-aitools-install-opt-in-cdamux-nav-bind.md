# PLAN-349 — opt-in cdamux nav bind (aitools + install.sh) + amend ADR-014

- **Status**: ready
- **PRD**: [[../prd/PRD-348-cdamux-opt-in-nav-bind-and-docs|PRD-348]]
- **GitHub**: [#349](https://github.com/chatondearu/ai-dev-kit/issues/349)
- **Date**: 2026-11-14

## Summary

Add default-off `cda.aitools.cdamux.navBind.enable` and `install.sh --cdamux-nav-bind` for the Alt-N → `cdamux nav` bind; amend ADR-014 to allow managed opt-in fragments.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-001-host-tools-vs-kit|ADR-001]] | No forced host install; opt-in companion / install flags only |
| [[../foam/decisions/ADR-003-cda-aitools|ADR-003]] | Versioned configs live under aitools; never enabled by plain install |
| [[../foam/decisions/ADR-014-cdamux-dedicated-navigator-session|ADR-014]] | Amend: opt-in managed bind OK; still switch-client only, no join-pane |

Do not violate accepted ADRs without superseding/amending them.

## Scope

### In scope

- Nix `navBind.enable` (default false) → managed tmux fragment / template
- `install.sh --cdamux-nav-bind` + uninstall
- ADR-014 amendment + short aitools/README note

### Out of scope

- Full docs sweep (#350)
- Alt-P bind (#297)
- Enabling by default

## Technical approach

Mirror `cdamux.hooks`: option in options schema; maid/tmux template gets an optional `# cdamux nav` bind block when enabled. Portable path: write a small managed conf under XDG (or `$CDA_AITOOLS_HOME`) and document/source it; `--uninstall` deletes the fragment and stops wiring it.

## Tasks

- [x] **ADR-014**: amend alternatives/consequences — opt-in managed fragment allowed (acceptance: ADR text updated, status still accepted or amended section dated)
- [x] **Nix**: `cda.aitools.cdamux.navBind.enable` default false wires `M-n` → `cdamux nav --client '#{client_name}'` (acceptance: option eval / template contains bind only when true)
- [x] **install.sh**: `--cdamux-nav-bind` install + uninstall removes managed snippet (acceptance: dry-run + install/uninstall test or scripted check)
- [x] **aitools/README**: document the option (acceptance: table row)
- [x] **Verify**: kit/aitools checks that apply; no forced bind in default template

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Clobber user tmux.conf | Managed fragment + source-file; never rewrite whole conf |
| Ghostty Option-as-Alt | Keep existing dogfood note for M- keys |

## Test plan

Seams: (1) option off → no bind, (2) option on → bind present, (3) install/uninstall portable.

- [x] Nix eval or template unit where practical
- [x] install.sh dry-run / temp HOME if pattern exists for hooks

## Verification

- [x] Lint / test / build per project conventions
- [x] Update foam index if ADR amendment needs a link
