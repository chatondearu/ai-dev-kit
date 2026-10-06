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
under `skills/` (e.g. `skills/core/cda-foam/` → `~/.cursor/skills/cda-foam/`).

Kit-owned workflow skills use the **`cda-`** prefix (chatondearu). Third-party
vendored skills keep their upstream names.

Each tool's install root can be overridden via its native env var (the default
is used otherwise): `CURSOR_HOME`, `CLAUDE_CONFIG_DIR`, `XDG_CONFIG_HOME`
(opencode), `AGENTS_HOME`.

## Layout

```
ai-dev-kit/
├── skills/                      # Agent-agnostic skills (nested by category)
│   ├── core/                    # cda-foam, cda-dev, cda-kanban, cda-agents, …
│   ├── dev/                     # nix-develop-shell, nix-direnv-setup
│   ├── quality/                 # git-commit, code-reviewer, technical-docs-sync
│   ├── design/                  # frontend-design, taste, guidelines, cda-design-md
│   └── tools/                   # context7
├── rules/                       # Global user rules → Cursor + Claude (assembled)
├── scripts/assemble-claude-md.sh
├── claude/                      # CLAUDE.header.md + generated CLAUDE.md
├── agents/                      # Cursor subagents
├── cursor/plugins/local/        # Cursor local plugins
├── docs/recommended.md          # External tools (ECC, Superdesign, …)
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
| `cda-agents` | core | Bootstrap / update `AGENTS.md` |
| `cda-foam` | core | Foam graph, ADRs, kanban import |
| `cda-prd` | core | PRD from Kanban issues |
| `cda-plan` | core | Implementation plans from PRDs |
| `cda-dev` | core | Meta-orchestrator: intake → foam/kanban → worktrees → verify/CI → PRs |
| `cda-kanban` | core | GitHub Projects Kanban |
| `nix-develop-shell` | dev | Run commands in `nix develop` |
| `nix-direnv-setup` | dev | Scaffold flake + direnv |
| `git-commit` | quality | Conventional Commits |
| `code-reviewer` | quality | Structured code review |
| `technical-docs-sync` | quality | Keep `doc/` aligned with code |
| `frontend-design` | design | Bold creative UI |
| `design-taste-frontend` | design | Anti-slop direction (Taste Skill) |
| `web-design-guidelines` | design | Vercel interaction / a11y checklist |
| `cda-design-md` | design | Curated brand `DESIGN.md` templates |
| `context7` | tools | Library docs via Context7 |

Archived (not installed): see [`archive/`](archive/).

Optional external tools (not vendored): [`docs/recommended.md`](docs/recommended.md).

Third-party attributions: [`THIRD_PARTY.md`](THIRD_PARTY.md).

## Default project workflow (foam + kanban)

Every repository should use:

1. **`AGENTS.md`** — portable agent contract (`cda-agents`)
2. **`foam/`** — ADRs, PRDs, plans (`cda-foam`)
3. **GitHub Kanban** — issues and board (`cda-kanban`)

Before any feature or technical/product/design decision, agents must read foam,
search open GitHub issues and PRs, and confront new proposals with existing
decisions.

### Bootstrap a new repo

```bash
bash ~/.cursor/skills/cda-agents/scripts/setup-agents.sh \
  --type auto --init-foam

# Feature pipeline (issue #42 example):
bash ~/.cursor/skills/cda-prd/scripts/new-prd.sh 42
bash ~/.cursor/skills/cda-plan/scripts/new-plan.sh 42

bash ~/.cursor/skills/cda-foam/scripts/import-kanban.sh
```

Multi-subject chat batches (intake → epic PRD → N child plans → parallel PRs):
use skill **`cda-dev`**.

### Refresh agent instructions after kit changes

```bash
./scripts/assemble-claude-md.sh && ./install.sh
bash ~/.cursor/skills/cda-agents/scripts/setup-agents.sh --type auto --force
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
