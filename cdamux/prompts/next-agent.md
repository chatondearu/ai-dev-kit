# cdamux — next agent session

**Mode**: v2 mission control TUI (PRD-41) + extract prep

**Epic**: https://github.com/chatondearu/ai-dev-kit/issues/40  
**Active**: [#58](https://github.com/chatondearu/ai-dev-kit/issues/58) — mission control dashboard  
**Plan**: `plans/PLAN-41-cdamux-mission-control-tui.md` (phases 1–6)  
**UX reference**: `prototype/dashboard-ui/index.html`  
**Extract**: [#56](https://github.com/chatondearu/ai-dev-kit/issues/56) (after dogfood + v2 or parallel)

## Pick work from GitHub

1. [#58](https://github.com/chatondearu/ai-dev-kit/issues/58) — follow PLAN-41 phase order (tree → composer → spawn → sidecar → tabs → polish)
2. [#56](https://github.com/chatondearu/ai-dev-kit/issues/56) — standalone repo extract (separate track)

## Orchestration

Only for new issues after v1:

```bash
bash ~/.cursor/skills/cda-dev/scripts/orch.sh \
  --mode ready-pickup --repo /hdd/dev/chatondearu/ai-dev-kit --issue <N>
```

## Do not

- Enable `cda.aitools.cdamux` in Nix until second-host dogfood (ADR-003)
- Remove skzv/ccmux templates from `aitools/`
- Wire `install.sh` (ADR-001)

## Verify

```bash
cd cdamux && cargo build && cargo test
cargo run -p cdamux-cli -- doctor
```

## Reference

`docs/research.md`, `plans/PLAN-39-cdamux-v0.md`, `prd/PRD-39-cdamux.md`, `docs/dogfood.md`
