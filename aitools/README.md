# cda.aitools — experimental companion

**Status: experimental.** Shared tmux / **cdamux** / Moshi / **Graphify** wiring for
all machines. See [ADR-003](../foam/decisions/ADR-003-cda-aitools.md) and
[ADR-008](../foam/decisions/ADR-008-graphify-code-graph.md).

Portable kit (`./install.sh`) stays **skills-only** ([ADR-001](../foam/decisions/ADR-001-host-tools-vs-kit.md)).
Session tools and configs are opt-in via **Nix** (`cda.aitools`).

## What lives here (source of truth)

| Path | Purpose |
| ---- | ------- |
| `templates/tmux.conf` | Shared tmux + cdamux popup; `detach-on-destroy on` (avoid `next` on tmux 3.7) |
| `templates/tmux-nav-bind.conf` | Opt-in Alt-N → `cdamux nav` (`cdamux.navBind.enable` / `install.sh --cdamux-nav-bind`) |
| `templates/tmux-persist.conf` | Optional resurrect/continuum fragment (`tmux.persist.enable`) |
| `templates/cdamux/config.toml` | cdamux config (`@PROJECTS_ROOT@` substituted by Nix) |
| `scripts/moshi-hook-bootstrap.sh` | Pair Moshi + install hooks (e.g. `--target cursor`) |
| `scripts/graphify-activate.sh` | Idempotent `graphify install` (throwaway cwd; shields symlinked CLAUDE.md) |
| `scripts/cdamux-align-mac.sh` | macOS: `cargo install` from in-tree `cdamux/` + config |
| `nix/` | `nixosModules.cdaAitools` + `darwinModules.cdaAitools` |

Build cdamux / graphify / Coolify CLI from the kit: `nix build .#cdamux`,
`nix build .#graphify`, `nix build .#coolify-cli`.

## Host Nix (thin overlay)

**NixOS** — `dev/aitools.nix` on neo-nix is the reference:

```nix
imports = [ inputs.ai-dev-kit.nixosModules.cdaAitools ];
cda.aitools = {
  enable = true;
  user = "chaton";
  deploy.enable = true;
  cdamux.projectsRoot = "/hdd/dev";
  tmux = {
    clipboard = "wayland";
    persist.enable = true;
  };
  mosh.enable = true;
  cdamuxd.enable = true;
  moshiHook = { enable = true; hookTargets = [ "cursor" ]; };
  # Optional code graph (ADR-008); off by default
  # graphify = { enable = true; installMethod = "auto"; claudeConfigDir = "/cda/.claude"; };
  # Official Coolify CLI; off by default (token via host env + coolify-env-sync)
  # coolify = { enable = true; baseUrl = "https://coolify.example.com"; };
  # foamHome = "/cda/foam";
  # Agent hooks (sub-agent ledger + session id), refreshed at each rebuild; off by default
  # cdamux.hooks = { enable = true; claudeConfigDir = "/cda/.claude"; };
  # Opt-in Alt-N → cdamux nav in managed tmux.conf (ADR-014); needs deploy.enable
  # cdamux.navBind.enable = true;
};
```

Legacy option names `ccmux.*` / `ccmuxd.*` are renamed automatically (see `mkRenamedOptionModule`).

**nix-darwin** — `darwinModules.cdaAitools`; `tmux.clipboard = "darwin"`,
`installMethod = "nix"` or `auto` (in-tree package). Optional `darwin.cdamuxdPath` if not using Nix.

### cdamux agent hooks (`cda.aitools.cdamux.hooks`)

| Option | Meaning |
| ------ | ------- |
| `cdamux.hooks.enable` | default `false`; activation oneshot runs `cdamux-hooks wire` (Claude Code, Cursor, opencode, Codex). Needs `user` |
| `cdamux.hooks.claudeConfigDir` | `CLAUDE_CONFIG_DIR` for activation (units do not inherit the shell); null = `~/.claude` |

Settings point at the stable `~/.local/share/cdamux/hooks` link (refreshed to the
current package on each run) with the packaged `node`, and any older
`/cdamux/hooks/` entry (e.g. from `install.sh --cdamux-hooks`) is replaced. darwin
re-runs it when cdamux changes; NixOS at login (or `systemctl --user restart
cda-cdamux-hooks`). Agents are never restarted: run `cdamux restart --all`.
Details: `cdamux/docs/subagents.md`.

### cdamux nav bind (`cda.aitools.cdamux.navBind`)

| Option | Meaning |
| ------ | ------- |
| `cdamux.navBind.enable` | default `false`; appends Alt-N (`M-n`) → `cdamux nav --client '#{client_name}'` to the managed tmux.conf. Needs `deploy.enable` |

Portable: `./install.sh --cdamux-nav-bind` links `templates/tmux-nav-bind.conf` under
`$CDA_AITOOLS_HOME` (default `~/.config/cda-aitools`) and adds a marked
`source-file` block in `~/.tmux.conf`. `--uninstall` removes both. Dogfood keys:
`cdamux/docs/dogfood.md`.

### Graphify (`cda.aitools.graphify`)

| Option | Meaning |
| ------ | ------- |
| `graphify.enable` | `CDA_GRAPHIFY=1` + activation oneshot |
| `graphify.installMethod` | `auto` \| `nix` \| `uv` \| `none` (like cdamux) |
| `graphify.claudeConfigDir` | `CLAUDE_CONFIG_DIR` for activation (units do not inherit the shell) |
| `foamHome` | optional `CDA_FOAM_HOME` |

`auto`/`nix` use `aitools/nix/graphify-package.nix` (pinned wheel; some grammars
relaxed). `auto`/`uv` can fall back to `uv tool install graphifyy`. Manual:
`bash aitools/scripts/graphify-activate.sh [--ensure-uv]`.
Claude PreToolUse hooks (`graphify claude install`) are not wired — cwd-scoped
writes; see ADR-008.

### Coolify CLI (`cda.aitools.coolify`)

| Option | Meaning |
| ------ | ------- |
| `coolify.enable` | default `false`; adds `coolify` + `coolify-env-sync` |
| `coolify.installMethod` | `auto` \| `nix` = pinned release; `none` = manual install |
| `coolify.baseUrl` | optional public URL → `COOLIFY_BASE_URL` (never put the token in Nix) |

The CLI stores contexts in `~/.config/coolify/config.json`. Bootstrap from host
env (token never enters the Nix store):

```bash
export COOLIFY_BASE_URL="https://coolify.example.com"
export COOLIFY_TOKEN="***"   # host secret / agenix — not in flake
coolify-env-sync             # or: coolify context add -d -f default "$COOLIFY_BASE_URL" "$COOLIFY_TOKEN"
coolify context verify
```

`coolify update` cannot replace a Nix store binary — bump the pin in
`coolify-cli-package.nix` (`version` + four asset hashes via
`nix store prefetch-file`).

## Parity

| Feature | NixOS | nix-darwin | Manual |
| ------- | ----- | ---------- | ------ |
| Packages (tmux, cdamux, moshi-hook, graphify, coolify) | yes | yes | `cdamux-align-mac.sh` / `uv tool install graphifyy` / upstream install script |
| Deploy configs (maid) | `deploy.enable` | `deploy.enable` | copy `templates/cdamux` |
| cdamuxd | systemd | launchd | `cdamuxd` in background; only useful with `[daemon] listen_tailnet = true`, otherwise the unit exits immediately |
| mosh server | `mosh.enable` | — | brew / apt |
| moshi-hook | systemd + hook oneshots | launchd | `moshi-hook service install` |
| graphify skill register | systemd oneshot | launchd oneshot | `graphify-activate.sh` |
| cdamux agent hooks | systemd oneshot | launchd oneshot | `install.sh --cdamux-hooks` |
| cdamux nav bind (Alt-N) | `cdamux.navBind` + deploy | same | `install.sh --cdamux-nav-bind` |

Secrets (Moshi token, SSH keys) stay **per host** — not in the kit.

## Moshi

```bash
bash aitools/scripts/moshi-hook-bootstrap.sh --target cursor
```

## Relocatable root

| Source | Meaning |
| ------ | ------- |
| `CDA_AITOOLS_HOME` | Env override |
| `cda.aitools.configRoot` | Nix option |
| Default | `~/.config/cda-aitools` |

## Layout

```
aitools/
├── README.md
├── prompts/bootstrap.md
├── scripts/
├── templates/
└── nix/
    ├── common.nix
    ├── module-core.nix
    ├── nixos.nix / darwin.nix
    ├── linux-services.nix / darwin-services.nix
    ├── maid-deploy.nix
    ├── moshi-hook-package.nix
    ├── graphify-package.nix
    └── coolify-cli-package.nix
```
