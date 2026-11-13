# PLAN-267 — Resolve foam/ and replace Notes tab with Foam browser

- **Status**: ready
- **PRD**: [[../prd/PRD-265-cdamux-foam-browser|PRD-265]]
- **GitHub**: [#267](https://github.com/chatondearu/ai-dev-kit/issues/267)
- **Blocked by**: #266
- **Date**: 2026-11-13

## Summary

Resolve the project’s `foam/` tree from the selected Fleet row (git toplevel /
discovered project) and replace the Notes sidecar tab with a read-only browser
listing `foam/**/*.md`.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| ADR from #266 (Foam browser) | Replace Notes sidecar; foam/ only; read-only; no Teep rail |
| [[../foam/decisions/ADR-004-cdamux|ADR-004]] | Local FS only |
| [[../foam/decisions/ADR-005-cda-graph-orchestration|ADR-005]] | Do not overload orch graph UI |

## Scope

### In scope

- Foam root resolution (prefer git toplevel over pane cwd)
- List markdown under `foam/`
- Retarget SideTab Notes → Foam (label + data source)
- Empty / missing foam empty state
- Snapshot / unit coverage for the new list path

### Out of scope

- Markdown rendering (#268)
- Deep-link (#269)
- Writing foam files
- Changing `cdamux notes` CLI behaviour

## Technical approach

Extend or parallel `cdamux-core` notes resolution with a foam-root helper; wire
Fleet sidecar load path to list foam entries. Reuse project discovery /
`git_toplevel` patterns already used by spawn/review. Keep CLI notes untouched.

## Tasks

- [ ] Add foam-root resolution from project/session path (git toplevel + `foam/`)
      with tests for cwd-vs-root and missing foam
- [ ] Expose a list of relative `foam/**/*.md` paths (deterministic order)
- [ ] Replace Notes sidecar data binding with Foam list + selection state
- [ ] Empty state when foam missing or empty
- [ ] Update Fleet snapshots / draw tests for the Foam tab label and list
- [ ] Document behaviour in `cdamux/docs/architecture.md` notes→foam pointer

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Worktree cwd resolves wrong root | Always git toplevel / discovered project |
| Large foam trees | Cap list size or lazy depth; document limit in UI |

## Test plan

- [ ] Unit: resolve foam root from nested cwd
- [ ] Unit/snapshot: Foam tab shows fixtures under a temp `foam/`
- [ ] Empty foam / missing dir covered

## Verification

- [ ] `nix develop -c bash scripts/verify.sh`
- [ ] PR `Closes #267`
