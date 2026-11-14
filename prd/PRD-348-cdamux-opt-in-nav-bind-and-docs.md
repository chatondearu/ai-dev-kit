# PRD-348 — opt-in cdamux nav bind + docs for Fleet hygiene/nav

- **Status**: approved
- **GitHub**: [#348](https://github.com/chatondearu/ai-dev-kit/issues/348)
- **Date**: 2026-11-14
- **Tags**: #product #architecture

## Problem

Epic #334 shipped `cdamux nav`, dismiss `d`, and kill hygiene, but:

1. Operators still must paste the `M-n` bind by hand; ADR-014 forbade kit install of Alt-N, while `aitools` already opt-in-wires hooks and tmux templates.
2. Docs across kit `doc/`, `cdamux/docs/`, and `aitools/` are uneven on nav vs deprecated sidebar and the new dismiss/hygiene flows.

## Goals

- [ ] Opt-in Alt-N → `cdamux nav` via `cda.aitools.cdamux.navBind.enable` (default false) and `install.sh --cdamux-nav-bind`
- [ ] Amend ADR-014: managed opt-in fragments allowed; still no forced bind
- [ ] Docs reflect nav, `d` dismiss, hygiene / `--sweep-orphans`, sidebar deprecation

## Non-goals

- Enabling nav bind by default
- Alt-P / `cdamux --pick` bind (tracked as #297)
- Editing personal home-manager outside this kit’s `aitools` / `install.sh` surfaces

## Users & scenarios

| Persona | Scenario |
| ------- | -------- |
| Nix aitools user | Sets `cdamux.navBind.enable = true`, rebuilds, `M-n` toggles navigator |
| Portable kit user | Runs `./install.sh --cdamux-nav-bind`, sources managed snippet, same bind |
| Operator reading docs | Finds nav keys, dismiss, and “sidebar deprecated” without contradictory primary guidance |

## Requirements

### Functional

- [ ] Nix option default false; when true, managed tmux config includes:
  `bind-key -n M-n run-shell "cdamux nav --client '#{client_name}'"`
- [ ] `install.sh --cdamux-nav-bind` installs a managed fragment; `--uninstall` removes it
- [ ] ADR-014 amended accordingly
- [ ] Docs updated for nav / dismiss / hygiene / sidebar deprecation + option names

### Non-functional

- [ ] Matches hooks pattern (opt-in, removable)
- [ ] Verify / doc checks green for touched paths

## Related project memory

- ADRs: [[../foam/decisions/ADR-001-host-tools-vs-kit|ADR-001]], [[../foam/decisions/ADR-003-cda-aitools|ADR-003]], [[../foam/decisions/ADR-014-cdamux-dedicated-navigator-session|ADR-014]] (amend)
- Prior: [[../prd/PRD-334-cdamux-fleet-hygiene-navigator|PRD-334]] shipped

## Open questions

1. Whether the install.sh fragment is sourced from `~/.tmux.conf` automatically or only documented — prefer a managed file under XDG + one documented `source-file` line, or maid-style when aitools deploy is on.

## Success metrics

- Fresh machine with opt-in: `M-n` opens `cdamux-nav` without join-pane into agents
- Docs search for “sidebar” does not present join-pane as the primary navigator

## Implementation pointer

[[../plans/PLAN-349-feat-aitools-install-opt-in-cdamux-nav-bind|PLAN-349]], [[../plans/PLAN-350-docs-cdamux-kit-nav-dismiss-hygiene|PLAN-350]]
