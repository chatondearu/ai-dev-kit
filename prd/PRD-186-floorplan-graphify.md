# PRD-186 — Floorplan map skill + graphify code graph across the kit

- **Status**: approved
- **GitHub**: [#186](https://github.com/chatondearu/ai-dev-kit/issues/186)
- **Date**: 2026-11-05
- **Tags**: #product #tooling #skills

## Problem

Agents spend most of a session re-reading a project before they can act. Two
external tools shorten that: Floorplan (an interactive architecture map built by
the agent) and Graphify (a tree-sitter code graph agents can query). Today
neither is wired into the kit: Floorplan is Claude/Codex-only and writes outside
project memory; Graphify must be installed and configured by hand on each host,
and no kit skill knows to use it.

## Goals

- [ ] Floorplan ships as a kit skill that works from any supported agent and
      files its map in project memory (foam) when there is one.
- [ ] Updating the kit's Nix companion on a host installs and configures
      Graphify, behind one switch.
- [ ] Every code-reading kit skill queries the graph when the switch is on.

## Non-goals

- Enabling Graphify in the maintainer's own nix-config (done by the user).
- cdamux integration of either tool.
- Teaching `install.sh` to install packages ([[../foam/decisions/ADR-001-host-tools-vs-kit|ADR-001]]).

## Users & scenarios

| Persona | Scenario |
| ------- | -------- |
| Maintainer in Cursor | "Make me a floorplan of this app" → map under `foam/floorplan/`, note `foam/floorplan.md` |
| Maintainer, repo without foam | Map lands in `$CDA_FOAM_HOME/projects/<repo>/floorplan/`, else `<repo>/floorplan/` |
| New host | `cda.aitools.graphify.enable = true` → graphify on PATH, `/graphify` skill registered, `CDA_GRAPHIFY=1` |
| cda-dev worker | Reads `graphify-out/GRAPH_REPORT.md` / queries the graph before grepping |

## Requirements

### Functional

- [ ] #187 — `skills/tools/cda-floorplan/` vendored from upstream v0.3.0 (MIT),
      host-agnostic skill dir resolution, Ask bar engines `agent`,
      `cursor-agent`, `claude`, `opencode`, `codex`; output order project
      `foam/` → `$CDA_FOAM_HOME/projects/<repo>/` → `<repo>/floorplan/`; no
      `CLAUDE.local.md` block.
- [ ] #188 — `cda.aitools.graphify.{enable,installMethod}` and
      `cda.aitools.foamHome`; pinned package with `uv` fallback; exports
      `CDA_GRAPHIFY=1`; runs `graphify install` (no cwd hooks); bootstrap prompt,
      doctor, recommended docs; ADR-008.
- [ ] #189 — shared rule in `rules/`, one line per code-reading skill, kiss
      SessionStart hook points at the graph report when on.

### Non-functional

- [ ] Off by default; with the switch off, kit behaviour is unchanged.
- [ ] kiss hooks stay local-only, fail-open, fast.
- [ ] `nix develop -c bash scripts/verify.sh` green.

## Related project memory

- ADRs: [[../foam/decisions/ADR-001-host-tools-vs-kit|ADR-001]],
  [[../foam/decisions/ADR-003-cda-aitools|ADR-003]], ADR-008 (new, #188)

## Open questions

1. Graphify Nix build may need tree-sitter grammars not in nixpkgs → `auto`
   falls back to `uv` (decided at intake).

## Success metrics

- A cda-dev worker on a graphify-enabled repo reads the report before its first grep.

## Implementation pointer

`plans/PLAN-187-cda-floorplan.md`, `plans/PLAN-188-aitools-graphify.md`,
`plans/PLAN-189-skills-graphify.md`
