# Skills layout

Skills are grouped by role. **Installers flatten** this tree: each directory
that contains `SKILL.md` is linked as `~/.cursor/skills/<skill-name>/` (and the
same name under Claude / opencode).

| Category | Path | Skills |
| -------- | ---- | ------ |
| **core** | `core/` | Project memory, batch orchestrator, kanban, PRD, plan, agent bootstrap |
| **dev** | `dev/` | Nix shell, direnv |
| **quality** | `quality/` | Review, commits, docs |
| **design** | `design/` | Frontend UI |
| **tools** | `tools/` | Context7, task CLI |

Add a new skill under the best-fitting category. Re-run `./install.sh` after
adding a `SKILL.md` — no installer edits required.
