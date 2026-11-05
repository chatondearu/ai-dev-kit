# ADR-004 — cdamux (in-tree until v0, then own repo)

- **Status**: accepted
- **Date**: 2026-10-26
- **Tags**: #architecture #tooling #deployment
- **GitHub**: [#40](https://github.com/chatondearu/ai-dev-kit/issues/40)

## Context

Operators dogfood **skzv/ccmux** and **agent-manager** for tmux-backed AI coding
agents. Neither fully matches the desired workflow:

- skzv/ccmux: orchestrator that spawns tmux sessions (good), but shallow project
  discovery (one level under `projects.root`), no dynamic `groupBy`, TUI stays open
  in tmux popups after attach.
- agent-manager: excellent spawn/review/quick-prompt and nested logical groups, but
  groups are manual trees (not filesystem `groupBy` like epilande/ccmux), separate
  tmux server (`agentmgr`), no first-class multi-root or folder browser.
- Pickers (MuxPilot, taimux, tmux-agent): great navigation, no spawn/orchestration.

Research and feature matrix: [`cdamux/docs/research.md`](../../cdamux/docs/research.md).

## Decision

1. **New product `cdamux`** — Rust workspace under repo path `cdamux/` (monorepo
   submodule-in-spirit until extractable).
2. **Architecture**: `cdamuxd` daemon (tmux state, HTTP on tailnet optional) +
   `cdamux` CLI (TUI dashboard + `--pick` popup mode that exits after attach).
3. **v0 scope** (see PRD-39 / PLAN-39): multi `projects.roots`, discover modes,
   configurable session `group_by`, new-session flow (agent + directory tree +
   custom path), tailnet API stub for future web UI.
4. **Relationship to ADR-003**: `aitools/` keeps shipping **skzv/ccmux** templates
   and Nix wiring; **cdamux is not enabled** from `install.sh` or default Nix
   modules until v0 dogfood. Optional future `cda.aitools.cdamux` overlay.
5. **Extraction**: When v0 is functional, move `cdamux/` to `github.com/chatondearu/cdamux`
   (or chosen org), replace in-tree path with `git submodule` or vendor pin; update
   this ADR with the new repo URL.

## Alternatives considered

| Alternative | Why rejected |
| ----------- | ------------ |
| Fork skzv/ccmux (Go) | FSL licence friction; large surface; we want Rust + slimmer v0 |
| Patch-only skzv | Upstream slow on multi-root / picker quit; still Go stack |
| agent-manager only | Missing dynamic groupBy and disk tree; separate tmux server |
| New repo day one | No dogfood in kit context; in-tree lowers friction |

## Consequences

- **Positive**: Single place for research matrix + product intent; agents can bootstrap
  from `cdamux/AGENTS.md`; aligns with “tmux is truth, tool orchestrates”.
- **Negative**: Maintenance of another binary; risk of duplicating ccmux/agent-manager
  unless v0 stays narrow.
- **Constraint**: Do not break ADR-001 (host tools outside kit install path).

## Amendment 2026-11 — tailnet API hardening

Context: KISS audit finding 8 ([#90](https://github.com/chatondearu/ai-dev-kit/issues/90),
[#96](https://github.com/chatondearu/ai-dev-kit/issues/96)). `POST /v1/sessions/:id/send`
drives agent shells; "tailnet only" was the sole guard and `CDAMUX_TAILNET_BIND_ADDR`
accepted `0.0.0.0`.

Decisions (status stays **accepted**):

1. **Unspecified binds are refused.** `CDAMUX_TAILNET_BIND_ADDR=0.0.0.0` / `::` is an
   error naming the variable; the tailnet server does not start. Rejected alternative:
   warn and bind anyway — it contradicts "no network except explicit tailnet bind".
2. **Shared token, env-driven.** `CDAMUX_TAILNET_TOKEN` set ⇒ every `/v1/*` request
   must carry `X-Cdamux-Token` (constant-time compare, 401 JSON otherwise). Unset ⇒
   open API + one startup warning, so existing dogfood setups keep working. Rotation
   = change env + restart. Rejected alternatives: per-peer Tailscale identity headers
   (needs `tailscale serve`/whois plumbing, out of v0 scope), a new auth dependency
   (ADR constraint: minimize dependencies). Static dashboard assets stay public; the
   dashboard reads the token from `localStorage.cdamux_token`.
3. **`?repo=` is canonicalized** and must resolve under a configured root; 404 bodies
   never echo the path (no directory-existence oracle).
4. **Token stays optional; requests are gated instead** ([#138](https://github.com/chatondearu/ai-dev-kit/issues/138)):
   `Host` must be the bind `ip:port` or the MagicDNS `name:port` (421), `POST` must be `application/json` (415,
   forces a CORS preflight that gets 405), bodies over 64 KiB get 413 and the token is
   checked before any body byte is read, `send` `line` rejects control characters (400).
   `cdamuxd` exits 1 when the server cannot start so the service manager restarts it.

Docs: [`cdamux/docs/tailnet-api.md`](../../cdamux/docs/tailnet-api.md).

## Amendment 2026-11-05 — session restore registry

[[ADR-009-cdamux-session-restore-registry|ADR-009]]: cdamux writes one JSON restore
snapshot per spawned session under `$XDG_STATE_HOME/cdamux/sessions/`. tmux stays the
truth for what runs; the registry is never read to decide liveness.

## References

- PRD: [`prd/PRD-39-cdamux.md`](../../prd/PRD-39-cdamux.md)
- Plan (v0, archived): [`foam/archive/plans/PLAN-39-cdamux-v0.md`](../../foam/archive/plans/PLAN-39-cdamux-v0.md)
- [[ADR-003-cda-aitools|ADR-003]] cda.aitools companion
