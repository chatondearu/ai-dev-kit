# cdamux dogfood checklist

Run on maintainer NixOS/Linux host with tmux + Ghostty.

## Prerequisites

- `tmux` ≥ 3.2
- Rust toolchain (`nix develop` at ai-dev-kit root, or system `cargo`)
- `cursor-agent` or configured agent in `~/.config/cdamux/config.toml`

## Config bootstrap

```bash
mkdir -p ~/.config/cdamux
cp /hdd/dev/chatondearu/ai-dev-kit/cdamux/config/cdamux.example.toml \
  ~/.config/cdamux/config.toml
# Edit paths under [[projects.roots]] for your machine.
```

Example roots in the template: `/hdd/dev`, `/home/chaton/etc/nixos`.

## Build

```bash
cd /hdd/dev/chatondearu/ai-dev-kit/cdamux
cargo build
cargo test
```

## Daemon (optional, enriches list + state cache)

```bash
cargo run -p cdamuxd
# Socket: $XDG_RUNTIME_DIR/cdamux/cdamux.sock
# Cache:  $XDG_CACHE_HOME/cdamux/sessions.json
```

## tmux popup bind (kit target)

```tmux
bind-key -n M-p display-popup -E -w 80% -h 75% "cdamux --pick"
```

Install the `cdamux` binary on `PATH` (e.g. `cargo install --path crates/cdamux-cli`).

## Scenarios (v0)

- [ ] Configure two `[[projects.roots]]` paths
- [ ] `cdamux doctor` passes (tmux, config, agent ids)
- [ ] `cdamux spawn --agent cursor --path <git-repo>` creates tmux session `c-<basename>`
- [ ] `cdamux spawn -i` — skim project tree + agent
- [ ] `cdamux list` shows `name`, `path`, `state` columns
- [ ] `cdamux --pick` from `tmux display-popup -E` closes popup after selection
- [ ] Repo at `org/nested/repo` appears when `discover = "git_only"` (`cdamux projects`)
- [ ] `list --group-by cwd` prints `##` group headers (text mode)
- [ ] `list --json --group-by project` returns grouped JSON

## Regression vs skzv/ccmux

- [ ] Can still use skzv/ccmux in parallel on same tmux server (document session name collisions on `c-*`)
- [ ] `aitools` templates unchanged until explicit switch

## Notes

Record frictions in GitHub issue [#40](https://github.com/chatondearu/ai-dev-kit/issues/40) comments.
