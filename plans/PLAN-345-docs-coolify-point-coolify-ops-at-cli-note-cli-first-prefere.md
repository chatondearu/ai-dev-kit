# PLAN-345 — docs(coolify): point coolify-ops at CLI + note CLI-first preference

- **Status**: draft
- **PRD**: [[../prd/PRD-343-coolify-cli-in-kit|PRD-343]]
- **GitHub**: [#345](https://github.com/chatondearu/ai-dev-kit/issues/345)
- **Date**: 2026-11-14

## Summary

Make coolify-ops CLI-primary and record CLI-over-MCP preference for agents in
kit docs. Depends on packaged CLI from #344.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-001-host-tools-vs-kit|ADR-001]] | Document companion/flake install; not install.sh |
| [[../foam/decisions/ADR-003-cda-aitools|ADR-003]] | Point operators at `cda.aitools.coolify` |

## Scope

### In scope

- Rewrite `cursor/plugins/local/coolify-ops` README, skills, commands for `coolify`
- Demote or remove curl/TS as the primary path
- Short notes in `AGENTS.md` Project-specific and `docs/recommended.md`

### Out of scope

- Nix packaging (PLAN-344)
- Shipping an MCP server

## Technical approach

After #344 lands (or on a branch that includes it): update coolify-ops templates
to call `coolify …`; link aitools/flake install + env bootstrap. Add 2–4 lines
in AGENTS.md and recommended.md: prefer CLIs for ops; do not invent MCP when a
CLI exists.

## Tasks

- [ ] Update coolify-ops README / skills / commands to CLI-first
- [ ] Remove or demote curl/TS primary examples
- [ ] AGENTS.md + docs/recommended.md CLI-over-MCP note
- [ ] Cross-link PLAN-344 package / env docs

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| #344 not merged yet | Blocked by #344; implement on top of that branch or after merge |
| Stale MCP wording elsewhere | Grep coolify/MCP in kit docs; fix only coolify/ops-related lines |

## Test plan

- [ ] Manual read-through of coolify-ops commands for accurate CLI invocations
- [ ] `bash scripts/verify.sh` if docs are covered by verify hooks

## Verification

- [ ] Lint / test / build per project conventions
- [ ] No new ADR unless preference warrants one (intake chose docs only)
