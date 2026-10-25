# cda.aitools — experimental companion

**Status: experimental.** Shared tmux / ccmux / Moshi wiring for all machines. See
[ADR-003](../foam/decisions/ADR-003-cda-aitools.md).

Portable kit (`./install.sh`) stays **skills-only** ([ADR-001](../foam/decisions/ADR-001-host-tools-vs-kit.md)).
Session tools and configs are opt-in via **Nix** (`cda.aitools`).

## What lives here (source of truth)

| Path | Purpose |
| ---- | ------- |
| `templates/tmux.conf` | Shared tmux + ccmux picker |
| `templates/tmux-persist.conf` | Optional resurrect/continuum fragment (`tmux.persist.enable`) |
| `templates/ccmux/config.toml` | ccmux daemon/agents (`@PROJECTS_ROOT@` substituted by Nix) |
| `templates/ccmux/ccmux.json` | Claude dirs sidecar |
| `scripts/moshi-hook-bootstrap.sh` | Pair Moshi + install hooks (e.g. `--target cursor`) |
| `scripts/ccmux-align-mac.sh` | macOS: brew skzv/ccmux + template sync |
| `nix/` | `nixosModules.cdaAitools` + `darwinModules.cdaAitools` |

## Host Nix (thin overlay)

**NixOS** — `dev/aitools.nix` on neo-nix is the reference:

```nix
imports = [ inputs.ai-dev-kit.nixosModules.cdaAitools ];
cda.aitools = {
  enable = true;
  user = "chaton";
  deploy.enable = true;
  ccmux.projectsRoot = "/hdd/dev";
  tmux = {
    clipboard = "wayland";
    persist.enable = true;
  };
  mosh.enable = true;
  ccmuxd.enable = true;
  moshiHook = { enable = true; hookTargets = [ "cursor" ]; };
};
```

**nix-darwin** — `darwinModules.cdaAitools`; `tmux.clipboard = "darwin"`,
`installMethod = "brew"` or `auto`, optional `darwin.ccmuxdPath` for Homebrew `ccmuxd`.

## Parity

| Feature | NixOS | nix-darwin | Manual |
| ------- | ----- | ---------- | ------ |
| Packages (tmux, ccmux, moshi-hook) | yes | yes | brew / scripts |
| Deploy configs (maid) | `deploy.enable` | `deploy.enable` | `ccmux-align-mac.sh` |
| ccmuxd | systemd | launchd | `ccmux daemon install` |
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
