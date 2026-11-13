# PRD-293 — Restore fast Alt-P / dashboard open

- **Status**: approved
- **GitHub**: [#293](https://github.com/chatondearu/ai-dev-kit/issues/293)
- **Date**: 2026-11-13
- **Tags**: #product #cdamux #performance

## Problem

Tmux Alt-P opens a popup running full `cdamux` (Mission Control). After the Foam
browser (ADR-012) and heavy `$CDAMUX_HOME` dogfood (hundreds of agents/runs),
first paint can feel slow. Diagnosis: binary startup is fine (~1 ms);
**eager Foam walk/render on every session focus** (even when Preview is
selected) and **unbounded run/agent listing** dominate under disk pressure.

## Goals

- [ ] First paint of popup/dashboard is snappy when the Foam tab is not selected.
- [ ] Large `$CDAMUX_HOME` (hundreds of JSON files) does not dominate open.
- [ ] Foam behaviour (ADR-012) unchanged when the Foam tab is used.
- [ ] Operators can measure time-to-first-paint (opt-in) and follow a dogfood
      checklist for Alt-P.
- [ ] Optional documented fast bind via `cdamux --pick` without changing default
      `M-p` unless opted in.

## Non-goals

- Rewriting Mission Control layout or Teep file rail.
- Changing default Alt-P to `--pick` for all kit users without opt-in.
- Full redesign of `$CDAMUX_HOME` schema.
- Auto-GC policy changes beyond what listing caps need (GC already exists).

## Users & scenarios

| Persona | Scenario |
| ------- | -------- |
| Maintainer in tmux | Alt-P → dashboard visible quickly on Preview; Foam still works when tabbed |
| Dogfood orch host | Hundreds of runs/agents under `$CDAMUX_HOME` → open still acceptable |
| Operator wanting switch-only | Opt-in bind uses `--pick` for instant switch-client |

## Requirements

### Functional

- [ ] Foam sidecar load/render/deep-link is deferred until Foam tab is active
      (or equivalent rule that keeps Preview-first open cheap).
- [ ] Run/agent listing on open is bounded / indexed so full-dir sort+parse is
      not required for every project before first paint.
- [ ] Opt-in timing (`CDAMUX_TIMING=1` or similar) logs major open phases.
- [ ] Docs/template mention optional `--pick` fast bind.

### Non-functional

- [ ] Verify green; TUI snapshots updated if behaviour changes.
- [ ] No Foam feature regression when Foam tab used (ADR-012).

## Children

| Issue | Scope |
| ----- | ----- |
| [#294](https://github.com/chatondearu/ai-dev-kit/issues/294) | Lazy Foam sidecar |
| [#295](https://github.com/chatondearu/ai-dev-kit/issues/295) | Bound run/agent listing |
| [#296](https://github.com/chatondearu/ai-dev-kit/issues/296) | First-paint timing + dogfood |
| [#297](https://github.com/chatondearu/ai-dev-kit/issues/297) | Optional `--pick` bind |

## Related project memory

- [[../foam/decisions/ADR-012-foam-browser-replaces-notes-sidecar|ADR-012]]
- [[../foam/decisions/ADR-004-cdamux|ADR-004]]
- [[../foam/decisions/ADR-011-cdamux-unified-state-model|ADR-011]]
- PRD-41 mission control; parents #40 / #58

## Open questions

_(Assumed)_ Deep-link applies when Foam is focused or when switching to Foam
with current preview text — implementer picks one rule and documents it in the
PR for #294.

## Success metrics

- Subjective: Alt-P on this kit repo feels near-instant again on Preview.
- Opt-in timings show foam work absent on Preview-first open.

## Implementation pointer

Child plans: `plans/PLAN-294-*.md` … `PLAN-297-*.md`.
