# Skills layout

Skills are grouped by role. **Installers flatten** this tree: each directory
that contains `SKILL.md` is linked as `~/.cursor/skills/<skill-name>/` (and the
same name under Claude / opencode).

Kit-owned workflow skills use the **`cda-`** prefix so they are easy to spot.

| Category | Path | Skills |
| -------- | ---- | ------ |
| **core** | `core/` | `cda-foam`, `cda-prd`, `cda-plan`, `cda-dev`, `cda-kanban`, `cda-agents` |
| **dev** | `dev/` | Nix shell, direnv |
| **quality** | `quality/` | Review, commits, docs |
| **design** | `design/` | `frontend-design`, taste, Vercel guidelines, `cda-design-md` |
| **tools** | `tools/` | Context7 |

Archived skills live under repo-root [`archive/`](../archive/) (not linked by `install.sh`).

Add a new skill under the best-fitting category. Re-run `./install.sh` after
adding a `SKILL.md` — no installer edits required.
