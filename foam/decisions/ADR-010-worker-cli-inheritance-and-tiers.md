# ADR-010 — Worker CLI inheritance and tiers

- **Status**: accepted
- **Date**: 2026-11-10
- **Tags**: #architecture #tooling #autonomy #cost

## Context

`orch.sh` picked the first agent CLI on `PATH` (`agent`, `cursor-agent`, `claude`,
`opencode`). A run started from a Claude Code session could spawn Cursor workers
(observed on the maintainer's Mac). Epic
[#206](https://github.com/chatondearu/ai-dev-kit/issues/206)
([[../../prd/PRD-206-cli-inheritance-tiers|PRD-206]]) wants workers on the CLI of the
session that launched the run, with explicit overrides per role.

## Decision

1. **Roles**: `worker`, `reviewer`, `verifier`, `integrator`, `pr-fix`. Each
   `orch_agent_run` call site passes its role as `ORCH_ROLE` (default `worker`; the
   pr-fix loop runs with `ORCH_ROLE=pr-fix`).
2. **Resolution order** (`orch_resolve_agent <role>` in `lib/agents.sh`), first hit wins:
   1. `ORCH_AGENT_<ROLE>` (upper case, `-` → `_`, e.g. `ORCH_AGENT_PR_FIX`) — `role-override`
   2. `ORCH_AGENT` / `--agent` — `override`
   3. parent CLI — `parent`: env markers first (`CLAUDECODE=1` → `claude`;
      `CODEX_THREAD_ID` / `CODEX_SESSION_ID` → `codex`; `CURSOR_INVOKED_AS`, set by the
      cursor-agent launcher → `agent` / `cursor-agent`), else the nearest ancestor
      process whose `comm`, interpreter or script basename is `claude`, `agent`,
      `cursor-agent`, `opencode` or `codex`. Accepted only if that CLI is on `PATH`.
   4. first of `agent`, `cursor-agent`, `claude`, `opencode` on `PATH` — `path`
3. **Session translation**: the session agent turns a user request ("verifier en
   codex", "tout en cursor") into `ORCH_AGENT*` env on the `orch.sh` command line;
   otherwise it sets nothing and inheritance applies.
4. **AFK requires an explicit CLI**: `orch-auto.sh` has no parent session, so it exits
   2 when `ORCH_AGENT` is empty (amends [[ADR-002-orch-auto-and-fast-lane|ADR-002]]).
5. **Visibility**: `orch.sh --dry-run` prints `agent[<role>]: <cli> (<source>)` per role.
6. `codex` runs as `codex exec -` (prompt on stdin). Permission flags of every CLI:
   [[ADR-019-worker-permission-boundary|ADR-019]] (no bypass flag; superseded 2026-11-23).

### Tiers (#208)

7. **Tier table**: `skills/core/cda-dev/config/tiers.tsv` maps `cli<TAB>tier<TAB>flags`
   for `claude`, `cursor` (`agent` / `cursor-agent`), `opencode` and `codex`; tiers are
   `small`, `medium`, `large`. `ORCH_TIERS_FILE` replaces the whole table; a missing or
   unreadable file is an error (`orch.sh` exits 2), never a silent fallback. opencode
   and codex rows are marked unverified until run on the maintainer host.
8. **Tier per role** (`orch_tier_for_role`): `ORCH_TIER` (current tier, set by
   escalation in #209) > `ORCH_TIER_<ROLE>` > default (`integrator` → `small`, other
   roles → `medium`). Tiers only add flags: they never change the resolved CLI.
9. **Pinned bypass**: an agent command that already passes `--model` / `-m` gets no
   tier flags and is recorded as tier `pinned` (no role tiers, no escalation).
   Default Cursor models live in `tiers.tsv`. Putting `--model` on
   `[agents.cursor] command` is this bypass, not the source of truth.
10. **Unknown CLI**: a CLI with no row runs without flags and warns once per run.
11. **Visibility**: `cli`, `model`, `effort`, `tier` are written to the worker entry of
    `.orch/run/status.json` (the worker's own agent, not its reviewer / verifier) and to
    graph nodes of agent runs;
    `--dry-run` prints `tier[<role>]: <tier> <flags>`.

### Launch paths (amendment 2026-11-25)

`orch.sh` applies `tiers.tsv` for the role tier and for escalation, unless that
command is already pinned. Interactive `cdamux worker spawn` uses
`[agents.cursor] command` and then the Cursor CLI picker; it does not read
`tiers.tsv` today. This decision does not add `cdamux worker spawn --model`.

Cursor rows (ids from `cursor-agent --list-models`): small `composer-2.5`, medium
`grok-4.7-high-fast`, large `grok-4.7-xhigh`. No Claude Opus, Fable, or Sonnet on
those rows. A `--model` that lives only on the cdamux command freezes interactive
spawns and does not change the flags `orch.sh` takes from the TSV.

### Escalation (#209)

12. **One tier up per failure**: in the worker / pr-fix loop (`orch_graph_worker`) a red
    verify or a failed grounding verifier makes the next iteration run one tier up
    (`orch_tier_escalate` exports `ORCH_TIER`), bounded by `ORCH_TIER_MAX` (default
    `large`, validated by `orch.sh`). A failure at the ceiling ends the worker `blocked`
    (`tier ceiling <tier> reached: <failure>`, loop rc 4) even with iterations left; on the
    last iteration `max iter exhausted` stays the reason. Pinned commands, CLIs with no
    table row and tiers outside `small|medium|large` never escalate. The review gate keeps
    its one fix iteration. Only the worker / pr-fix calls use the escalated or task tier:
    the grounding verifier runs with `ORCH_TIER` cleared, so it keeps its own role tier.
13. **Task tier**: a graph-plan task may set `tier`; its worker starts there.

## Alternatives considered

| Alternative | Why rejected |
| ----------- | ------------ |
| Keep PATH order, document `ORCH_AGENT` | The reported bug: silent CLI switch from the launching session |
| Env markers only | Cursor/opencode markers are not documented; ancestry covers them |
| Guess a CLI for orch-auto from PATH | AFK picks a CLI nobody chose; a clear failure is cheaper |

## Consequences

- **Positive**: workers match the launching session; per-role overrides allow a
  cross-vendor second opinion (e.g. verifier on codex) without code changes.
- **Negative**: existing AFK env files without `ORCH_AGENT` now fail until it is set.
  A `node`-hosted CLI with no env marker and no recognisable script path falls back
  to `PATH`. Env markers are inherited, so they name the *outermost* marked CLI: a
  `cursor-agent` started under a Claude session (or in a tmux server first started
  from one) still sees `CLAUDECODE=1` and resolves `claude`; set `ORCH_AGENT` there.
  The parent is resolved at each agent call, so a run that outlives its launching
  session falls back to `PATH` for later roles.
- **Constraint**: never install a CLI ([[ADR-001-host-tools-vs-kit|ADR-001]]).
  Kit `[agents.cursor] command` stays `cursor-agent` (no `--model`). `--model` on
  that command is the pinned bypass for the process that executes it: interactive
  spawn stays on one model (no role tiers, no escalation). `orch.sh` does not read
  that command; it still picks small/medium/large from `tiers.tsv`.

## References

- [[../../plans/PLAN-207-worker-cli-inheritance|PLAN-207]], issue
  [#207](https://github.com/chatondearu/ai-dev-kit/issues/207)
- [[../../plans/PLAN-208-model-tiers|PLAN-208]], issue
  [#208](https://github.com/chatondearu/ai-dev-kit/issues/208)
- [[../../plans/PLAN-209-tier-escalation|PLAN-209]], issue
  [#209](https://github.com/chatondearu/ai-dev-kit/issues/209)
- `skills/core/cda-dev/scripts/lib/agents.sh`, `orch.sh`, `orch-auto.sh`,
  `skills/core/cda-dev/config/tiers.tsv`
- Operator docs: [`doc/orchestration.md`](../../doc/orchestration.md),
  [`doc/configuration.md`](../../doc/configuration.md)
