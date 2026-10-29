# cdamux — next agent session

**Mode**: v2 mission control shipped (PRD-41) — maintenance, dogfood, extract

**Epic**: https://github.com/chatondearu/ai-dev-kit/issues/40  
**Dashboard**: [#58](https://github.com/chatondearu/ai-dev-kit/issues/58) — phases 1–6 complete (merge stacked PRs #65–#70)  
**Plan**: `plans/PLAN-41-cdamux-mission-control-tui.md`  
**UX reference**: `prototype/dashboard-ui/index.html`  
**Extract**: [#56](https://github.com/chatondearu/ai-dev-kit/issues/56) (after second-host dogfood)

## Pick work from GitHub

1. Dogfood v2 checklist in `docs/dogfood.md` → sign-off comment on [#58](https://github.com/chatondearu/ai-dev-kit/issues/58)
2. [#56](https://github.com/chatondearu/ai-dev-kit/issues/56) — standalone repo extract
3. Bugs/frictions from Fleet (`?` lists keys) → new issues under [#40](https://github.com/chatondearu/ai-dev-kit/issues/40)

## Orchestration

Only for new issues after v1:

```bash
bash ~/.cursor/skills/cda-dev/scripts/orch.sh \
  --mode ready-pickup --repo /hdd/dev/chatondearu/ai-dev-kit --issue <N>
```

## Do not

- Re-enable skzv/ccmux in `aitools/` without an ADR
- Wire `install.sh` (ADR-001)

## Verify

```bash
cd cdamux && cargo build && cargo test
cargo run -p cdamux-cli -- doctor
```

## Reference

`docs/research.md`, `foam/archive/plans/PLAN-39-cdamux-v0.md`, `prd/PRD-39-cdamux.md`, `docs/dogfood.md`
