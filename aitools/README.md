# cda.aitools — experimental companion

**Status: experimental.** Shared tmux / **cdamux** / Moshi wiring for all machines. See
[ADR-003](../foam/decisions/ADR-003-cda-aitools.md).

Portable kit (`./install.sh`) stays **skills-only** ([ADR-001](../foam/decisions/ADR-001-host-tools-vs-kit.md)).
Session tools and configs are opt-in via **Nix** (`cda.aitools`).

## What lives here (source of truth)

| Path | Purpose |
| ---- | ------- |
| `templates/tmux.conf` | Shared tmux + `cdamux --pick` popup |
| `templates/tmux-persist.conf` | Optional resurrect/continuum fragment (`tmux.persist.enable`) |
| `templates/cdamux/config.toml` | cdamux config (`@PROJECTS_ROOT@` substituted by Nix) |
| `scripts/moshi-hook-bootstrap.sh` | Pair Moshi + install hooks (e.g. `--target cursor`) |
| `scripts/cdamux-align-mac.sh` | macOS: `cargo install` from in-tree `cdamux/` + config |
| `nix/` | `nixosModules.cdaAitools` + `darwinModules.cdaAitools` |

Build cdamux from the kit: `nix build .#cdamux` (flake at repo root).

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
};
```

Legacy option names `ccmux.*` / `ccmuxd.*` are renamed automatically (see `mkRenamedOptionModule`).

**nix-darwin** — `darwinModules.cdaAitools`; `tmux.clipboard = "darwin"`,
`installMethod = "nix"` or `auto` (in-tree package). Optional `darwin.cdamuxdPath` if not using Nix.

## Parity

| Feature | NixOS | nix-darwin | Manual |
| ------- | ----- | ---------- | ------ |
| Packages (tmux, cdamux, moshi-hook) | yes | yes | `cdamux-align-mac.sh` |
| Deploy configs (maid) | `deploy.enable` | `deploy.enable` | copy `templates/cdamux` |
| cdamuxd | systemd | launchd | `cdamuxd` in background |
| mosh server | `mosh.enable` | — | brew / apt |
| moshi-hook | systemd + hook oneshots | launchd | `moshi-hook service install` |

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
    └── moshi-hook-package.nix
```
