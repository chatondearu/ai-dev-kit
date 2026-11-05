# PLAN-187 — Vendor floorplan as agent-agnostic cda-floorplan

- **Status**: implemented
- **PRD**: [[../prd/PRD-186-floorplan-graphify|PRD-186]]
- **GitHub**: [#187](https://github.com/chatondearu/ai-dev-kit/issues/187)
- **Date**: 2026-11-05

## Summary

Copy upstream Floorplan v0.3.0 (`haydenschmitty/floorplan`, MIT,
`skills/floorplan/`) into `skills/tools/cda-floorplan/`, make it run from any
host agent, and file the map in project memory.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-001-host-tools-vs-kit|ADR-001]] | Skill only; no package install from `install.sh` (Node is already a host prerequisite) |

## Scope

### In scope

- Vendored copy pinned to v0.3.0 (record commit sha), `THIRD_PARTY.md`, `skills/README.md`
- Skill name `cda-floorplan`; SKILL.md resolves its own dir without `${CLAUDE_SKILL_DIR}`
- Ask bar engines: `agent`, `cursor-agent`, `claude`, `opencode` (orch.sh order) + `codex`; `--engine` accepts all
- Output resolution: project `foam/` → `$CDA_FOAM_HOME/projects/<repo>/` → `<repo>/floorplan/`
- Foam note `floorplan.md` (wikilinks-friendly), `.gitignore` for generated files inside foam
- Remove `--claude-md` / `CLAUDE.local.md` block

### Out of scope

- Upstream sync automation; cdamux; graphify (#188/#189)

## Technical approach

Output root decided in one shared helper in `scripts/lib/` (e.g. `outdir.mjs`)
used by every script that today hardcodes `<root>/floorplan`. Engine detection
lives in `scripts/lib/agent.mjs` and `assets/serve.mjs`.

## Tasks

- [x] Copy upstream tree at v0.3.0, add license + THIRD_PARTY entry
- [x] Rename skill, rewrite SKILL.md host notes (Claude / Cursor / opencode / Codex)
- [x] Output dir helper + call sites + foam note + gitignore
- [x] Engine detection + `--engine` values
- [x] Drop CLAUDE.local.md block
- [x] `node` assert self-check (`scripts/tests/` or skill-local test wired into verify)
- [x] `skills/README.md` index row

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Hardcoded `floorplan/` paths in many scripts | grep every occurrence; single helper |
| Cursor/opencode CLI flags for read-only one-shot differ | document per engine; fall back to `claude` when absent |

## Test plan

- [x] Self-check: 3 output-resolution cases (foam, CDA_FOAM_HOME, fallback)
- [x] Self-check: engine detection order and `--engine` override
- [ ] Manual: build a map of this repo → `foam/floorplan/` (needs full agent describe pass; deferred)

## Verification

- [x] `nix develop -c bash scripts/verify.sh`
- [ ] Update foam feature list after merge
