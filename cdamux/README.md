# cdamux

Rust orchestrator for **AI coding agent sessions on tmux**: spawn, classify, group,
and pick sessions — with multi-root project discovery, a default **ratatui** dashboard (`cdamux`), and popup-friendly `--pick`.

**Status**: bootstrap / v0 in progress. Lives **in-tree** under [ai-dev-kit](https://github.com/chatondearu/ai-dev-kit) until functional; then [extract](docs/extract-repo.md) to its own repo (ADR-004).

## Documentation

| Doc | Purpose |
| --- | ------- |
| [docs/research.md](docs/research.md) | Competitor matrix & operator requirements |
| [docs/architecture.md](docs/architecture.md) | Daemon, CLI, API sketch |
| [docs/dogfood.md](docs/dogfood.md) | Manual test checklist |
| [AGENTS.md](AGENTS.md) | Instructions for coding agents |
| [../prd/PRD-39-cdamux.md](../prd/PRD-39-cdamux.md) | Product requirements |

## Build

```bash
cargo build
cargo test
./target/debug/cdamux doctor
```

Example config: copy `config/cdamux.example.toml` to `~/.config/cdamux/config.toml`.

## tmux popup

```tmux
bind-key -n M-p display-popup -E -w 80% -h 75% "cdamux"
```

Minimal skim picker only: `cdamux --pick` (exits right after attach).

## Licence

MIT OR Apache-2.0 (to be added at extract); see ai-dev-kit policy until then.

## Related kit pieces

- **cdamux** is the default in [`aitools/`](../aitools/README.md) (`nix build .#cdamux`).
