# ADR-014 — cdamux dedicated navigator session (replaces join-pane sidebar)

- **Status**: accepted
- **Date**: 2026-11-14
- **Tags**: #architecture #tooling #cdamux
- **GitHub**: [#337](https://github.com/chatondearu/ai-dev-kit/issues/337) · epic [#334](https://github.com/chatondearu/ai-dev-kit/issues/334)

## Context

`cdamux sidebar` (#321) injected a Fleet pane into the active agent session via
`split-window` / `join-pane`. That multiplied session chrome when Alt-P / fullscreen
cdamux also opened, and broke multi-client layouts when the pane moved. Operators
wanted a navigator that stays available without living inside agent sessions.

## Decision

1. **Navigator = dedicated tmux session** `cdamux-nav` (hidden from Fleet like
   home `cdamux`), running `cdamux nav --in-session`.
2. **Switch only** — `cdamux nav` / Alt-N example bind use `switch-client`; never
   `join-pane` into agent sessions. Agent pane counts stay unchanged when opening
   the navigator.
3. **`cdamux sidebar` is deprecated** — CLI stub prints redirect to `cdamux nav`;
   join-pane is no longer the primary dogfood path. Alt-P popup is unchanged.
4. **Return target** — `@cdamux_nav_return` on the nav session remembers where
   toggle / `q` should land.

## Alternatives considered

| Alternative | Why rejected / amended |
| ----------- | ---------------------- |
| Keep join-pane sidebar as primary | Contradicts PRD-334; layout / multi-client pain |
| Reuse home session `cdamux` as navigator | Home is the full dashboard entry; nav is a focused Fleet tree with a distinct toggle |
| Force Alt-N into arbitrary user tmux.conf from plain `install.sh` | Still rejected — violates ADR-001 (no forced host writes) |
| Opt-in managed fragment (`cda.aitools.cdamux.navBind.enable` / `install.sh --cdamux-nav-bind`) | **Accepted amendment (2026-11-14)** — same opt-in posture as hooks (ADR-003 / ADR-007); default off; removable; still `switch-client` only, never `join-pane` |

## Consequences

- **Positive**: Agent layouts stay clean; one live navigator session; clear dogfood bind (`M-n`).
- **Negative**: Operators still on `M-s` need dogfood + deprecation message.
- **Constraint**: Session hierarchy (ADR-007) and state model (ADR-011) unchanged;
  navigator is another session, same Fleet readers.
- **Amendment (2026-11-14)**: Managed opt-in fragments under aitools / `install.sh
  --cdamux-nav-bind` may wire `M-n` → `cdamux nav --client '#{client_name}'`. Plain
  install stays off; disabling / `--uninstall` removes the fragment.

## References

- [[ADR-001-host-tools-vs-kit|ADR-001]], [[ADR-003-cda-aitools|ADR-003]], [[ADR-004-cdamux|ADR-004]], [[ADR-007-cdamux-session-hierarchy|ADR-007]], [[ADR-011-cdamux-unified-state-model|ADR-011]]
- [[../../prd/PRD-334-cdamux-fleet-hygiene-navigator|PRD-334]], [[../../prd/PRD-348-cdamux-opt-in-nav-bind-and-docs|PRD-348]]
- [[../../plans/PLAN-337-feat-cdamux-dedicated-navigator-session|PLAN-337]], [[../../plans/PLAN-349-feat-aitools-install-opt-in-cdamux-nav-bind|PLAN-349]]
