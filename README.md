# ai-dev-kit

Shareable, version-controlled **AI / dev configuration** — Cursor skills, rules,
agents and local plugins (with room for Claude, Codex and VSCode) — installable
on any machine via a portable `install.sh` **or** declaratively with **Nix
(nix-maid)**.

This repo is the **single source of truth**. Your `~/.cursor` (and other tools)
link back to it, so edits are versioned and portable across setups.

## Layout

```
ai-dev-kit/
├── cursor/                 # → linked into ~/.cursor
│   ├── skills/             #   per-skill directories (SKILL.md + assets)
│   ├── user-rules/         #   global user rules
│   ├── agents/             #   custom subagents
│   └── plugins/local/      #   local Cursor plugins
├── claude/                 # Claude config (CLAUDE.md, etc.) — placeholder
├── codex/                  # Codex config — placeholder
├── vscode/                 # VSCode settings/snippets — placeholder
├── nix/maid.nix            # nix-maid file declarations (auto-discovers cursor/*)
├── flake.nix               # nix-maid package + NixOS module
└── install.sh              # portable symlink installer (no Nix)
```

Only the **immediate children** of `cursor/{skills,user-rules,agents,plugins/local}`
are linked, so assets installed by Cursor itself are never clobbered.

## Install — portable (any OS, no Nix)

```bash
./install.sh --dry-run     # preview
./install.sh               # create symlinks into ~/.cursor (backs up conflicts)
./install.sh --force       # overwrite conflicts instead of backing up
./install.sh --uninstall   # remove only the symlinks we created
```

Override the target with `--cursor-home DIR` or `CURSOR_HOME=…`.

## Install — Nix (NixOS, via nix-maid)

Use **one** mechanism per machine (don't mix with `install.sh` on the same host).

### As a NixOS module

```nix
# flake.nix inputs
inputs.ai-dev-kit.url = "github:chatondearu/ai-dev-kit";

# configuration.nix
imports = [ inputs.ai-dev-kit.nixosModules.default ];
aiDevKit = {
  enable = true;
  user = "chaton";
  # where the repo is checked out (live-edit friendly; {{home}} allowed)
  repoPath = "{{home}}/dev/chatondearu/ai-dev-kit";
};
```

### Standalone

```bash
nix build .#default
nix-env -if ./nix-maid-result && activate   # see nix-maid docs
```

`nix/maid.nix` derives the linked entries from `cursor/` at eval time, so adding
a skill/rule/agent/plugin needs no Nix edits.

## Included skills

| Skill | Purpose |
|-------|---------|
| `github-kanban-orchestrator` | Repo-agnostic GitHub Projects (v2) Kanban orchestration (milestones, issues, PRs, multi-agent). Auto-detects repo/project/field IDs via `gh`. |
| `nix-develop-shell` | Wrap project commands in `nix develop -c`. |
| `technical-docs-sync` | Keep `doc/` aligned with code and runtime config. |

Plus the `coolify-ops` local plugin.

## Requirements

- **Portable path**: `bash`, `coreutils`. The kanban skill also needs `gh` (with
  `project` scope) and `jq`.
- **Nix path**: flakes enabled; `nix-maid` is fetched as a flake input.

## Adding a new asset

1. Drop it under `cursor/skills/<name>/`, `cursor/user-rules/`, `cursor/agents/`,
   or `cursor/plugins/local/<name>/`.
2. Re-run `./install.sh` (or rebuild on Nix). Done.
