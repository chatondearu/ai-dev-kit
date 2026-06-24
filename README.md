# ai-dev-kit

Shareable, version-controlled **AI / dev configuration** — agent skills, rules,
subagents and local plugins — installable on any machine and **reusable across
agents** (Cursor, Claude Code, opencode…) via a portable `install.sh` **or**
declaratively with **Nix (nix-maid)**.

This repo is the **single source of truth**. Each agent's config dir links back
to it, so edits are versioned and shared everywhere at once.

## Why it works across agents

Skills use the common **`SKILL.md`** format (YAML frontmatter `name` +
`description`, markdown body). Every major agent discovers skills the same way —
only the install directory differs:

| Agent | Global skills dir |
|-------|-------------------|
| Cursor | `~/.cursor/skills/<name>/SKILL.md` |
| Claude Code | `~/.claude/skills/<name>/SKILL.md` |
| opencode | `~/.config/opencode/skills/` (also reads `~/.claude/skills`, `~/.agents/skills`) |
| universal | `~/.agents/skills/<name>/SKILL.md` |

So the **same** `skills/` tree is linked into each agent you use.

## Layout

```
ai-dev-kit/
├── skills/                 # CANONICAL, agent-agnostic skills (SKILL.md) → all agents
│   ├── github-kanban-orchestrator/
│   ├── nix-develop-shell/
│   └── technical-docs-sync/
├── rules/                  # global user rules            → Cursor (~/.cursor/user-rules)
├── agents/                 # custom subagents             → Cursor (~/.cursor/agents)
├── cursor/plugins/local/   # Cursor local plugins         → Cursor
├── claude/ codex/ vscode/  # tool-specific placeholders
├── nix/maid.nix            # nix-maid declarations (auto-discovers the trees)
├── flake.nix               # nix-maid package + NixOS module (aiDevKit.tools)
└── install.sh              # portable multi-agent symlink installer (no Nix)
```

Only the **immediate children** of each tree are linked, so assets a tool
installs itself are never clobbered.

## Install — portable (any OS, no Nix)

```bash
./install.sh --dry-run     # preview
./install.sh               # link into every detected agent
./install.sh --force       # overwrite conflicts instead of backing up
./install.sh --uninstall   # remove only the symlinks we created
```

Tool selection (a tool is auto-enabled when its config dir exists; Cursor is
always on; `agents` is opt-in):

```bash
./install.sh --claude --opencode      # force on
./install.sh --no-claude              # force off
./install.sh --agents                 # also link ~/.agents/skills (universal)
./install.sh --cursor-home /path/.cursor
```

## Install — Nix (NixOS, via nix-maid)

Use **one** mechanism per machine (don't mix with `install.sh` on the same host).

### NixOS module

```nix
# flake inputs
inputs.ai-dev-kit.url = "github:chatondearu/ai-dev-kit";

# configuration.nix
imports = [ inputs.ai-dev-kit.nixosModules.default ];
aiDevKit = {
  enable = true;
  user = "chaton";
  tools = [ "cursor" "claude" "opencode" ];   # which agents get the skills
  repoPath = "{{home}}/dev/chatondearu/ai-dev-kit";  # live-edit friendly
};
```

### Standalone

```bash
nix build .#default        # builds the nix-maid activation package
# then follow nix-maid activation (nix-env -if … && activate)
```

`nix/maid.nix` derives entries from the repo at eval time, so adding a
skill/rule/agent/plugin needs no Nix edits.

## Included skills

| Skill | Purpose |
|-------|---------|
| `github-kanban-orchestrator` | Repo-agnostic GitHub Projects (v2) Kanban orchestration (milestones, issues, PRs, multi-agent). Auto-detects repo/project/field IDs via `gh` (`scripts/gh-board.sh`). |
| `nix-develop-shell` | Wrap project commands in `nix develop -c`. |
| `technical-docs-sync` | Keep `doc/` aligned with code and runtime config. |
| `code-reviewer` | Structured code review for local changes or remote PRs. |
| `context7` | Fetch up-to-date library docs via the Context7 API. |
| `frontend-design` | Build distinctive, production-grade frontend UIs. |
| `git-commit` | Conventional Commits with diff analysis and smart staging. |
| `task-management` | CLI to track feature subtasks, dependencies and status. |

Plus the `coolify-ops` Cursor local plugin.

> `code-reviewer`, `context7`, `frontend-design`, `git-commit` and
> `task-management` were imported from a prior opencode setup and kept as-is
> (their original licenses are preserved in each `SKILL.md`).

## Requirements

- **Portable path**: `bash`, `coreutils`. The kanban skill also needs `gh`
  (with `project` scope) and `jq`.
- **Nix path**: flakes enabled; `nix-maid` is fetched as a flake input.

## Adding a new asset

1. Drop it under `skills/<name>/`, `rules/`, `agents/`, or
   `cursor/plugins/local/<name>/`.
2. Re-run `./install.sh` (or rebuild on Nix). Done — it propagates to every
   enabled agent.
