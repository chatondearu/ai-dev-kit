# ai-dev-kit

Shareable, version-controlled **AI / dev configuration** — agent skills, rules,
subagents and local plugins — installable on any machine and **reusable across
agents** (Cursor, Claude Code, opencode…) via a portable `install.sh` **or**
declaratively with **Nix (nix-maid)**.

This repo is the **single source of truth**. Each agent's config dir links back
to it, so edits are versioned and shared everywhere at once.

## Why it works across agents

| Asset | Cursor | Claude Code | opencode | Portable |
| ----- | ------ | ----------- | -------- | -------- |
| **Skills** (`SKILL.md`) | `~/.cursor/skills/` | `~/.claude/skills/` | `~/.config/opencode/skills/` | Yes |
| **Rules** (`rules/*.md`) | `~/.cursor/user-rules/` | `~/.claude/CLAUDE.md` (assembled) | — | Partial |
| **AGENTS.md** | per repo | per repo | per repo | Yes |
| **Subagents** | `~/.cursor/agents/` | — | — | Cursor only |
| **Plugins** | `~/.cursor/plugins/local/` | — | — | Cursor only |

Skills use the common **`SKILL.md`** format. Installers **flatten** nested folders
under `skills/` (e.g. `skills/core/foam-project-memory/` → `~/.cursor/skills/foam-project-memory/`).

Each tool's install root can be overridden via its native env var (the default
is used otherwise): `CURSOR_HOME`, `CLAUDE_CONFIG_DIR`, `XDG_CONFIG_HOME`
(opencode), `AGENTS_HOME`.

## Layout

```
ai-dev-kit/
├── skills/                      # Agent-agnostic skills (nested by category)
│   ├── core/                    # foam, orchestrator, kanban, project-agents-setup
│   ├── dev/                     # nix-develop-shell, nix-direnv-setup
│   ├── quality/                 # git-commit, code-reviewer, technical-docs-sync
│   ├── design/                  # frontend-design
│   └── tools/                   # context7, task-management
├── rules/                       # Global user rules → Cursor + Claude (assembled)
├── scripts/assemble-claude-md.sh
├── claude/                      # CLAUDE.header.md + generated CLAUDE.md
├── agents/                      # Cursor subagents
├── cursor/plugins/local/        # Cursor local plugins
├── nix/maid.nix
└── install.sh
```

## Install — portable (any OS, no Nix)

```bash
./install.sh --dry-run     # preview
./install.sh               # link into every detected agent
./install.sh --force       # overwrite conflicts instead of backing up
./install.sh --claude      # include ~/.claude/CLAUDE.md from rules/
```

## Install — Nix (NixOS, via nix-maid)

```nix
aiDevKit = {
  enable = true;
  user = "chaton";
  tools = [ "cursor" "claude" "opencode" ];
  repoPath = "{{home}}/dev/chatondearu/ai-dev-kit";
};
```

Run `./scripts/assemble-claude-md.sh` before Nix eval if you changed `rules/`.

## Included skills

| Skill | Category | Purpose |
| ----- | -------- | ------- |
| `project-agents-setup` | core | Bootstrap / update `AGENTS.md` |
| `foam-project-memory` | core | Foam graph, ADRs, kanban import |
| `foam-prd` | core | PRD from Kanban issues |
| `foam-plan` | core | Implementation plans from PRDs |
| `dev-orchestrator` | core | Meta-orchestrator: intake → foam/kanban → worktrees → verify/CI → PRs |
| `foam-batch-orchestrator` | core | **Alias** → `dev-orchestrator` (deprecated name) |
| `github-kanban-orchestrator` | core | GitHub Projects Kanban |
| `nix-develop-shell` | dev | Run commands in `nix develop` |
| `nix-direnv-setup` | dev | Scaffold flake + direnv |
| `git-commit` | quality | Conventional Commits |
| `code-reviewer` | quality | Structured code review |
| `technical-docs-sync` | quality | Keep `doc/` aligned with code |
| `frontend-design` | design | Production-grade UI |
| `context7` | tools | Library docs via Context7 |
| `task-management` | tools | Feature subtask CLI |

## Default project workflow (foam + kanban)

Every repository should use:

1. **`AGENTS.md`** — portable agent contract (`project-agents-setup`)
2. **`foam/`** — ADRs, PRDs, plans (`foam-project-memory`)
3. **GitHub Kanban** — issues and board (`github-kanban-orchestrator`)

Before any feature or technical/product/design decision, agents must read foam,
search open GitHub issues and PRs, and confront new proposals with existing
decisions.

### Bootstrap a new repo

```bash
bash ~/.cursor/skills/project-agents-setup/scripts/setup-agents.sh \
  --type auto --init-foam

# Feature pipeline (issue #42 example):
bash ~/.cursor/skills/foam-prd/scripts/new-prd.sh 42
bash ~/.cursor/skills/foam-plan/scripts/new-plan.sh 42

bash ~/.cursor/skills/foam-project-memory/scripts/import-kanban.sh
```

Multi-subject chat batches (intake → epic PRD → N child plans → parallel PRs):
use skill **`dev-orchestrator`** (`foam-batch-orchestrator` is a deprecated alias).


### Refresh agent instructions after kit changes

```bash
./scripts/assemble-claude-md.sh && ./install.sh
bash ~/.cursor/skills/project-agents-setup/scripts/setup-agents.sh --type auto --force
```

## Global rules (Cursor + Claude)

| File | Topic |
| ---- | ----- |
| `rules/00-communication.md` | French replies, Vue/Nuxt, commits |
| `rules/01-project-workflow.md` | Mandatory foam + kanban on every project |
| `rules/nix-develop-shell.md` | Nix develop wrapper |

Claude receives the same content in `claude/CLAUDE.md` (assembled).

## Adding a new asset

1. Add `skills/<category>/<name>/SKILL.md`, or a file under `rules/`, `agents/`,
   or `cursor/plugins/local/<name>/`.
2. Re-run `./install.sh` (and `assemble-claude-md.sh` if rules changed).

## Requirements

- **Portable path**: `bash`, `coreutils`. Kanban and foam skills need `gh` and `jq`.
- **Nix path**: flakes enabled; `nix-maid` as flake input.
