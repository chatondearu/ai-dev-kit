# PLAN-207 — Resolve the worker CLI: overrides, parent session, PATH

- **Status**: ready
- **PRD**: [[../prd/PRD-206-cli-inheritance-tiers|PRD-206]]
- **GitHub**: [#207](https://github.com/chatondearu/ai-dev-kit/issues/207)
- **Date**: 2026-11-09

## Summary

Workers run the CLI of the session that launched `orch.sh`, unless a role or
global override says otherwise. PATH detection becomes the last resort.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-001-host-tools-vs-kit|ADR-001]] | Never install a CLI |
| [[../foam/decisions/ADR-002-orch-auto-and-fast-lane|ADR-002]] | orch-auto stays opt-in; now requires `ORCH_AGENT` |

## Scope

### In scope

- Roles: `worker`, `reviewer`, `verifier`, `integrator`, `pr-fix`. Pass the role
  as `ORCH_ROLE` at each `orch_agent_run` call site (`lib/loop.sh` reviewer,
  `lib/graph.sh` worker / verifier / integrator ×2, `orch.sh` pr-fix), default `worker`.
- `lib/agents.sh`: `orch_resolve_agent <role>` →
  1. `ORCH_AGENT_<ROLE>` (upper, `-` → `_`, e.g. `ORCH_AGENT_PR_FIX`)
  2. `ORCH_AGENT`
  3. parent CLI: `CLAUDECODE=1` → `claude`; `CODEX_THREAD_ID`/`CODEX_SESSION_ID` → `codex`;
     else walk `ps -o ppid=,comm=` ancestry (portable macOS/Linux) for `claude`,
     `agent`, `cursor-agent`, `opencode`, `codex` (basename match, stop at pid 1)
  4. existing PATH order
  Keep `orch_detect_agent` working for callers (`headless.sh`).
- `_orch_agent_exec` gains a `codex` case: `codex exec` reading the prompt (check
  `codex exec --help` in docs; stub-tested).
- `orch.sh --dry-run` prints `agent[<role>]: <cli> (<source>)` for each role,
  source ∈ `role-override|override|parent|path`.
- `orch-auto.sh`: exit 2 with a clear message when `ORCH_AGENT` is empty; update
  `templates/systemd/cda-orch-auto.env.example` (uncomment, explain).
- `cda-dev` SKILL.md "When to use the harness": the session agent translates a
  user request ("verifier en codex", "tout en cursor") into `ORCH_AGENT*` env on
  the `orch.sh` command line; otherwise sets nothing.
- New `foam/decisions/ADR-010-worker-cli-inheritance-and-tiers.md` (+ index,
  ADR-002 note); child #208 appends the tier section.
- Docs: `doc/orchestration.md`, `doc/configuration.md` (`ORCH_AGENT_<ROLE>`, `ORCH_ROLE`).

### Out of scope

- Model/effort flags (#208), escalation (#209).

## Tasks

- [ ] `ORCH_ROLE` at every call site
- [ ] `orch_resolve_agent` + parent detection with an injectable ps hook for tests
- [ ] codex exec case
- [ ] dry-run print
- [ ] orch-auto guard + env example
- [ ] SKILL.md, docs, ADR-010

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Cursor/opencode `comm` is `node` | match full command (`ps -o args=`) basename tokens too; fall back to PATH |
| Breaking existing AFK units | explicit error text naming the env file |

## Test plan

- [ ] `tests/test_agents.sh`: role override wins; global override; `CLAUDECODE=1` → claude even when `cursor-agent` is first on PATH; fake ps chain → cursor; nothing → PATH
- [ ] orch-auto without `ORCH_AGENT` exits non-zero (stubbed)
- [ ] `nix develop -c bash scripts/verify.sh`
