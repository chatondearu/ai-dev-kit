# cdamux — next agent session

**Mode**: maintenance + extract prep (post–#48 dogfood sign-off)

**Epic**: https://github.com/chatondearu/ai-dev-kit/issues/40  
**Milestone**: `cdamux-v1` (remaining: #45–#47)

## Pick work from GitHub

1. [#45](https://github.com/chatondearu/ai-dev-kit/issues/45) — static web UI (tailnet)
2. [#46](https://github.com/chatondearu/ai-dev-kit/issues/46) — hunk review → agent
3. [#47](https://github.com/chatondearu/ai-dev-kit/issues/47) — notes markdown per project

Extract (human + follow-up issue): `cdamux/docs/extract-repo.md`

## Orchestration

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
```

## Reference

`docs/research.md`, `plans/PLAN-39-cdamux-v0.md`, `prd/PRD-39-cdamux.md`, `docs/dogfood.md`
