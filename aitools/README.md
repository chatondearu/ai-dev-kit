# cda.aitools — experimental companion

**Status: experimental.** Dogfood on nix-darwin first; not a stable primary
install path until validated on ≥2 hosts. See foam
[ADR-003](../foam/decisions/ADR-003-cda-aitools.md).

Opt-in Nix companion for **tmux / ccmux** session configs and relocatable
roots. The portable kit (`./install.sh`) stays **config-only** and never
enables this module ([ADR-001](../foam/decisions/ADR-001-host-tools-vs-kit.md)).

## Relocatable root

| Source | Meaning |
| ------ | ------- |
| `CDA_AITOOLS_HOME` | Env override (highest priority at runtime) |
| `cda.aitools.configRoot` | Nix module option (e.g. `/cda/aitools`) |
| Default | `$XDG_CONFIG_HOME/cda-aitools` or `~/.config/cda-aitools` |

## Nix (opt-in)

```nix
# nix-darwin
imports = [ inputs.ai-dev-kit.darwinModules.cdaAitools ];
cda.aitools = {
  enable = true;
  configRoot = "/cda/aitools";  # optional
  installMethod = "auto";       # auto | nix | brew | none
};

# NixOS
imports = [ inputs.ai-dev-kit.nixosModules.cdaAitools ];
# same options under cda.aitools
```

`installMethod`:

- **auto** — prefer Nix package when available; on Darwin fall back to documenting brew for ccmux
- **nix** — Nix only
- **brew** — Darwin/Homebrew path for ccmux (never required on NixOS)
- **none** — configs only; no package install

## Bootstrap other machines

Use the agent prompt [`prompts/bootstrap.md`](prompts/bootstrap.md) — **not** a
project skill in the `install.sh` flatten.

## Orch worktrees

Kanban worktrees stay under `<repo>/.orch/worktrees/`. Cleanup:

```bash
bash skills/core/cda-dev/scripts/orch-cleanup.sh --repo /path/to/repo --dry-run
```

No automatic bridge to ccmux worktrees (v1).

## Layout

```
aitools/
├── README.md
├── prompts/bootstrap.md
├── templates/tmux.conf
├── templates/ccmux/               # placeholders / notes
└── nix/
    ├── common.nix                 # shared options
    └── module.nix                 # nixos + darwin entry
```
