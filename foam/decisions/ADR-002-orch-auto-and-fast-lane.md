# ADR-002 — Orch auto poller and fast-lane pickup

- **Status**: accepted
- **Date**: 2026-10-18
- **Tags**: #architecture #tooling #autonomy

## Context

We want AFK autonomy: clear GitHub tickets become CI-green PRs without a human
sitting in the IDE. [ADR-001](ADR-001-host-tools-vs-kit.md) already splits
**headless** (one prompt + verify, no Kanban/PR) from **orch** (Kanban,
worktrees, PRs). Extending headless with `--issue` / PR open would blur that
boundary for no gain — `ready-pickup` already does ticket → PR.

Two gaps remain:

1. **Heavy artifacts** — `ready-pickup` docs assumed PRD + `plans/PLAN-*` even
   when triage already wrote a durable agent brief (bugs / bounded scope).
2. **Manual dispatch** — someone must run `orch.sh --mode ready-pickup`; there
   is no opt-in poller that can be started/stopped so a laptop stays free.

## Decision

1. **Fast-lane** — If Status Ready and no `plans/PLAN-<id>-*` exists, the
   contract is the triage **agent brief** (`LANE=fast`). If a plan exists,
   `LANE=full` (plan + optional PRD) as today. No brief and no plan → block.
2. **Auto poller** — `skills/core/cda-dev/scripts/orch-auto.sh` discovers Ready
   issues (skip `triage:ready-for-human`, skip open PR on the expected branch),
   then calls `orch.sh --mode ready-pickup`. One run at a time (`flock`).
3. **Service opt-in** — Ship **templates** for systemd user `service` + `timer`
   (not enabled by `install.sh`). Operator toggles with
   `systemctl --user start|stop cda-orch-auto.timer`. Prefer a dedicated host
   (clone + `gh` + agent CLI) when the laptop should stay free.
4. **Headless unchanged** — still no issue/PR/worktree path (ADR-001).

## Alternatives considered

| Alternative | Why rejected |
| ----------- | ------------ |
| Teach `headless.sh` `--issue` + open PR | Contradicts ADR-001; duplicates orch |
| Always require PRD/plan | Blocks clear bugs that already have an agent brief |
| Always-on kit daemon from `install.sh` | Occupies host by default; fights ADR-001 host boundary |
| GitHub Actions as the only runner | Agent CLIs + secrets are awkward on GHA; local/dedicated host is simpler |

## Consequences

- **Positive**: Clear tickets can AFK to a mergeable PR; operator controls load
  via systemd start/stop; headless stays a light local tool.
- **Negative**: Fast-lane quality depends on triage brief discipline; mis-triaged
  cards can still be picked up.
- **Constraint**: No auto-merge. Units stay templates unless the operator copies
  and enables them.

## References

- Operator docs: [`doc/orchestration.md`](../../doc/orchestration.md),
  [`doc/architecture.md`](../../doc/architecture.md)
- `skills/core/cda-dev/scripts/orch.sh`, `orch-auto.sh`
- `skills/core/cda-dev/templates/systemd/`
- `skills/core/cda-triage/` (agent brief → Ready)
- [[ADR-001-host-tools-vs-kit|ADR-001]]
