# PLAN-44 — cdamux tailnet HTTP API

- **Status**: done
- **PRD**: [[../../prd/PRD-39-cdamux|PRD-39]] (v0.2)
- **GitHub**: [#44](https://github.com/chatondearu/ai-dev-kit/issues/44)
- **Date**: 2026-10-26

## Summary

When `listen_tailnet` is true, `cdamuxd` serves JSON HTTP on the Tailscale interface
for health, session list, pane tail, and send.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../../foam/decisions/ADR-004-cdamux|ADR-004]] | No public `0.0.0.0` bind by default |
| ADR-001 | Optional host tool |

## Scope

### In scope

- Endpoints per `cdamux/docs/architecture.md` API sketch
- Config `tailnet_port`

### Out of scope

- OAuth; static web (#45)

## Technical approach

Minimal HTTP server crate in `cdamuxd` (e.g. `tiny-http` or `axum` — justify deps);
resolve tailnet IP via `tailscale ip -4` or documented env override for tests.

## Tasks

- [x] `GET /v1/health` — version, tmux reachable
- [x] `GET /v1/sessions` — same shape as grouped list JSON
- [x] `GET /v1/sessions/:id/pane` — tail text (bounded buffer)
- [x] `POST /v1/sessions/:id/send` — JSON body line + optional enter
- [x] Bind only tailnet IP when `listen_tailnet = true`
- [x] Operator curl examples in `cdamux/docs/tailnet-api.md`

## Test plan

- **Seams**: handler unit tests with mock session source; bind disabled in CI default

## Verification

- [x] `cargo test`; manual curl on tailnet host (see `dogfood.md` sign-off)
