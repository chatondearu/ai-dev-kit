# PLAN-247 — Add orch new, orchestrator skill and worker preamble

- **Status**: draft
- **PRD**: [[../prd/PRD-239-orchestrator-session|PRD-239]]
- **GitHub**: [#247](https://github.com/chatondearu/ai-dev-kit/issues/247)
- **Blocked by**: #246
- **Date**: 2026-11-12

## Summary

Slice 2 orchestrator entrypoint: `cdamux orch new`, spawn-modal orchestrator
role, `cda-orchestrator` skill, and worker preamble that reports via `cdamux msg`.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-011-cdamux-unified-state-model|ADR-011]] | §1 orchestrator owns long-lived run; host mandatory |
| [[../foam/decisions/ADR-007-cdamux-session-hierarchy|ADR-007]] | Spawn modal / hierarchy; hooks stay opt-in |
| [[../foam/decisions/ADR-001-host-tools-vs-kit|ADR-001]] | Skill ships in kit; does not install client CLIs |
| [[../foam/decisions/ADR-010-worker-cli-inheritance-and-tiers|ADR-010]] | Workers inherit orch CLI unless overridden |

Do not violate accepted ADRs without superseding them.

## Scope

### In scope

- `cdamux orch new [--agent X]`
- Spawn modal *orchestrator* role
- Skill `cda-orchestrator` (spawn/track, relay rules, ask escalation, read by `ref`)
- Worker preamble using `cdamux msg` (patterns from `cda-dev/prompts/worker.md`)

### Out of scope

- Fleet glyph/unread UI (slice 4)
- orch.sh model cut (slice 3)
- Native wake upgrades

## Technical approach

`orch new` is a thin spawn with `role=orchestrator`, a fresh long-lived run id, and
skill wiring so the client loads `cda-orchestrator`. Worker preamble is injected
or documented as the default prompt prefix for `worker spawn`.

## Tasks

- [ ] **orch new**: starts client with `role=orchestrator`, long-lived run, `CDAMUX_AGENT_ID`, skill available (acceptance: CLI + state file assertions on temp home / throwaway tmux)
- [ ] **Spawn modal**: *orchestrator* role mirrors `orch new` (acceptance: unit/UI test or modal contract test matching existing spawn modal patterns)
- [ ] **Skill `cda-orchestrator`**: documented behaviours — spawn/track workers, relay (decides / `@worker` forces / one-line confirm), answer or escalate asks, never paste raw output, read reports by `ref` (acceptance: skill file present + referenced from orch new / docs)
- [ ] **Worker preamble**: blockers/questions/completion via `cdamux msg` (acceptance: preamble text shipped and used by `worker spawn` default path)
- [ ] **Docs**: brief operator note in cdamux docs linking orch/worker/msg
- [ ] **Verify**: `nix develop -c bash scripts/verify.sh` green

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Skill not discovered by all clients | Follow existing skill install/link patterns per client; document |
| Modal + CLI drift | Single helper builds orchestrator spawn args for both |

## Test plan

Seams: (1) orch new state/role/run, (2) modal role wiring, (3) skill+preamble presence, (4) verify.

- [ ] Throwaway tmux orch new smoke
- [ ] Skill path / preamble content checks
- [ ] `nix develop -c bash scripts/verify.sh`

## Verification

- [ ] Lint / test / build per project conventions
- [ ] Update foam / ADR only if a new decision was made
