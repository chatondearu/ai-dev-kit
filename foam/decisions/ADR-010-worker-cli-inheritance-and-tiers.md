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
6. `codex` runs as `codex exec --dangerously-bypass-approvals-and-sandbox -` (prompt on
   stdin), like `claude -p --dangerously-skip-permissions`.

Tiers (model/effort per role) are added by #208 as a further section of this ADR.

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

## References

- [[../../plans/PLAN-207-worker-cli-inheritance|PLAN-207]], issue
  [#207](https://github.com/chatondearu/ai-dev-kit/issues/207)
- `skills/core/cda-dev/scripts/lib/agents.sh`, `orch.sh`, `orch-auto.sh`
- Operator docs: [`doc/orchestration.md`](../../doc/orchestration.md),
  [`doc/configuration.md`](../../doc/configuration.md)
