# cdamux — next agent session

**Epic**: https://github.com/chatondearu/ai-dev-kit/issues/40  
**Milestone**: `cdamux-v1` (PRD-39 through v1)

## Pick work from GitHub (ordered)

1. [#41](https://github.com/chatondearu/ai-dev-kit/issues/41) — session state, doctor, attach exclusive, spawn polish  
2. [#42](https://github.com/chatondearu/ai-dev-kit/issues/42) — ratatui dashboard  
3. [#43](https://github.com/chatondearu/ai-dev-kit/issues/43) — send without attach  
4. [#44](https://github.com/chatondearu/ai-dev-kit/issues/44) — tailnet HTTP API  
5. [#45](https://github.com/chatondearu/ai-dev-kit/issues/45) — static web UI  
6. [#46](https://github.com/chatondearu/ai-dev-kit/issues/46) — hunk review → agent  
7. [#47](https://github.com/chatondearu/ai-dev-kit/issues/47) — notes markdown per project  
8. [#48](https://github.com/chatondearu/ai-dev-kit/issues/48) — dogfood + PRD closure + extract prep  

## Orchestration

```bash
bash ~/.cursor/skills/cda-dev/scripts/orch.sh \
  --mode ready-pickup --repo /hdd/dev/chatondearu/ai-dev-kit --issue <N>
```

## Do not

- Add Nix module to `aitools/` until #48 + second-host dogfood
- Remove skzv/ccmux templates

## Verify

```bash
cd cdamux && cargo build && cargo test
```

## Reference

`docs/research.md`, `plans/PLAN-39-cdamux-v0.md`, `prd/PRD-39-cdamux.md`
