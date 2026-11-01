# cdamux — agent guide

Read this file first when working in `cdamux/` (in-tree until v0 extract).

## Mandatory context

1. **PRD**: [`../prd/PRD-39-cdamux.md`](../prd/PRD-39-cdamux.md) · v2: [`../prd/PRD-41-cdamux-mission-control-tui.md`](../prd/PRD-41-cdamux-mission-control-tui.md)
2. **Plan (active)**: [`../plans/PLAN-41-cdamux-mission-control-tui.md`](../plans/PLAN-41-cdamux-mission-control-tui.md)
3. **Plan (v0 bootstrap, archived)**: [`../foam/archive/plans/PLAN-39-cdamux-v0.md`](../foam/archive/plans/PLAN-39-cdamux-v0.md)
4. **ADR**: [`../foam/decisions/ADR-004-cdamux.md`](../foam/decisions/ADR-004-cdamux.md)
5. **Research matrix**: [`docs/research.md`](docs/research.md) — do not drop competitor rationale
6. **Architecture**: [`docs/architecture.md`](docs/architecture.md)
7. **GitHub**: [#40](https://github.com/chatondearu/ai-dev-kit/issues/40) · v2 epic [#58](https://github.com/chatondearu/ai-dev-kit/issues/58)

Parent kit rules: [`../AGENTS.md`](../AGENTS.md) (foam, kanban, verify scripts for kit — not all apply here).

## Product intent (one paragraph)

**cdamux** orchestrates AI agent sessions on **tmux**: spawn with chosen agent and path,
multi-root project discovery, dynamic `group_by` on sessions (epilande-style), and a
**`--pick`** mode that exits after `switch-client` for tmux popups. Optional
tailnet daemon/web later. Rust. Default session tool in `aitools/` (replaces ccmux).

## Repo layout

```text
cdamux/
  crates/cdamux-core/   # config, discover, types
  crates/cdamuxd/       # daemon (tailnet API server only; list/TUI read tmux directly)
  crates/cdamux-cli/    # user binary `cdamux`
  config/               # example TOML
  docs/                 # research, architecture, dogfood
```

## Build & test

```bash
cd cdamux
cargo build
cargo test
cargo run -p cdamux-cli -- doctor
```

Use `nix develop` at ai-dev-kit root if Rust toolchain comes from the flake.

## Testing against tmux

A worker once killed the operator's live tmux server. Rules:

- Throwaway servers only: `TMUX_TMPDIR=$(mktemp -d)` **and** `tmux -L <name>` on
  every tmux command you type. cdamux itself calls `tmux` without `-L`, so
  `TMUX_TMPDIR` (or a `tmux` wrapper in `PATH` adding `-L <name>`) is what
  isolates the binary under test.
- Before any `kill-server`: `tmux -L <name> display -p '#{socket_path}'`; abort
  if it is not under your temp dir or contains `/default`.
- Never touch the default server, never `pkill -f tmux`, never `kill 0`.

## Implementation order (PLAN-39)

1. Config load + `GroupBy` / `DiscoverMode` types in `cdamux-core`
2. `cdamux list` delegating to tmux (read directly from tmux; no daemon IPC)
3. `cdamux --pick` → switch-client → exit 0
4. `discover` git repos under all roots
5. `spawn` with agent table from config
6. `cdamuxd` tailnet API server

## Conventions

- Comments in English; user-facing CLI strings English for v0 (i18n later).
- Licence: MIT OR Apache-2.0 at repo root when extracted.
- Minimize dependencies; no telemetry crates.
- Do not wire into `install.sh` or enable in `aitools` Nix by default.

## New session bootstrap

If starting cold, read [`prompts/next-agent.md`](prompts/next-agent.md) for the current sprint task.
