# PLAN-39 — cdamux v0 bootstrap

- **Status**: done
- **PRD**: [[../prd/PRD-39-cdamux|PRD-39]]
- **GitHub**: [#40](https://github.com/chatondearu/ai-dev-kit/issues/40)
- **Date**: 2026-10-26

## Summary

Scaffold Rust workspace under `cdamux/`, capture research in docs, implement
minimal daemon + CLI stubs, then iterate toward pick/spawn/multi-root per PRD-39.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-001-host-tools-vs-kit|ADR-001]] | Not wired into `install.sh` |
| [[../foam/decisions/ADR-003-cda-aitools|ADR-003]] | skzv/ccmux remains default in `aitools/` until cdamux dogfood |
| [[../foam/decisions/ADR-004-cdamux|ADR-004]] | In-tree until v0; then extract repo |

## Scope

### In scope (this plan)

- Workspace layout, `AGENTS.md`, research + architecture docs
- Config schema (`config.example.toml`)
- `cdamuxd` stub: load config, log ready
- `cdamux` stub: clap subcommands `list`, `attach`, `spawn`, `doctor`, global `--pick`
- CI-friendly `cargo test` / `cargo build`

### Out of scope (follow-up plans)

- Full ratatui TUI dashboard
- Web UI + tailnet HTTP
- Agent hook installers
- Nix package in `aitools`

## Technical approach

```
cdamux/
  crates/cdamux-cli/     # clap entry, TUI later
  crates/cdamuxd/        # daemon, tmux poll later
  crates/cdamux-core/    # config, types, discover (shared)
  docs/research.md
  docs/architecture.md
```

tmux remains source of truth. Daemon classifies sessions; CLI talks to daemon
via Unix socket (JSON lines or HTTP over UDS).

## Tasks

- [x] ADR-004, PRD-39, PLAN-39, issue #40
- [x] `cdamux/` workspace + docs + AGENTS.md
- [x] `cdamux-core`: parse `config.toml`, `discover` flat + `git_only` (+ test)
- [x] CLI stubs: `doctor`, `list`, `projects`, `attach`, `spawn`/`--pick` placeholders
- [x] `cdamuxd` stub binary
- [x] `cdamuxd`: tmux `list-sessions` parse, state file under XDG
- [x] `cdamux`: `list`/`attach` via `tmux` CLI (delegate v0)
- [x] `cdamux --pick`: skim or ratatui list → `switch-client` → exit 0
- [x] `group_by` in list output (text columns first)
- [x] Spawn: `cursor-agent` / `opencode` from config + `--path`
- [x] Dogfood doc in `cdamux/docs/dogfood.md`
- [x] Extract checklist validated in #48 (`cdamux/docs/extract-repo.md`); repo creation tracked separately

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Scope creep (12 agents) | Config-driven command table; 2 agents v0 |
| Duplicate ccmuxd | Narrow API; reuse tmux naming `c-*` optional |
| Nix packaging later | `flake.nix` in cdamux when repo splits |

## Verify

```bash
cd cdamux && cargo build && cargo test
cdamux doctor
```
