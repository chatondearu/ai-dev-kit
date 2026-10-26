# PLAN-45 — cdamux static web UI (tailnet)

- **Status**: in-progress
- **PRD**: [[../prd/PRD-39-cdamux|PRD-39]] (v0.2)
- **GitHub**: [#45](https://github.com/chatondearu/ai-dev-kit/issues/45)
- **Date**: 2026-10-26

## Summary

Minimal browser UI on tailnet: session list, pane preview, send form — consumes #44 API.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-004-cdamux|ADR-004]] | No telemetry; tailnet-only |

## Scope

### In scope

- Static assets served by `cdamuxd` or small `cdamux-web` crate
- SSH/Mosh operator doc

### Out of scope

- Native mobile app

## Technical approach

Vanilla HTML/JS or minimal build; embed or `include_dir` at compile time; CORS
only same origin.

## Tasks

- [ ] Page lists sessions with state from API
- [ ] View pane tail for selected session
- [ ] Send message form → POST send endpoint
- [ ] Document Mosh/SSH attach path in `cdamux/docs/`
- [ ] Build integrated in `cargo build` or documented one-liner

## Test plan

- Manual on tailnet; optional headless fetch test for static asset route

## Verification

- [ ] API #44 merged; browser smoke on maintainer tailnet
