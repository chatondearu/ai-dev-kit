---
title: "feat(cda-kiss): port selected upstream ponytail fixes (since v5.2.0)"
status: done
priority: P2
updated: 2026-11-23
---

## Goal

Bring the upstream ponytail fixes that apply to `cda-kiss` since the v5.2.0 import
(`2961b52`), without the band/board UI, the bench or the ledger.

## Acceptance

- [x] Session map drops repo-controlled dir names that are not path-like or longer than 100 chars (upstream `5c5ad8a`); test with a hostile folder name
- [x] Map `git ls-files` buffer 4 → 64 MiB (upstream `f7bfbb9`)
- [x] UTF-8-safe stdin in `activate.js` / `mode-tracker.js` (upstream `ebd8654`); split-BOM test
- [x] `mode-tracker.js` and `opencode/kiss.mjs` honour `CDA_ROLE=orchestrator` (routing only); tests
- [x] `cda-kiss-audit` shortcut debt ledger section (from `ponytail-debt`, upstream `84740e3`)
- [x] #237 closed with upstream Cursor evidence (upstream `67a1601`)

Left out on purpose: A/B bench and per-session ledger (separate decisions).
