# Skills layout

Skills are grouped by role. **Installers flatten** this tree: each directory
that contains `SKILL.md` is linked as `~/.cursor/skills/<skill-name>/` (and the
same name under Claude / opencode).

Kit-owned workflow skills use the **`cda-`** prefix so they are easy to spot.

| Category | Path | Skills |
| -------- | ---- | ------ |
| **core** | `core/` | `cda-explore`, `cda-triage`, `cda-foam`, `cda-prd`, `cda-plan`, `cda-dev`, `cda-kanban`, `cda-agents` |
| **dev** | `dev/` | Nix shell, direnv |
| **quality** | `quality/` | Review, commits, docs, `tdd`, `diagnosing-bugs`, `review-standards-spec`, `grilling`, `codebase-design`, `improve-codebase-architecture`, `writing-for-agents`, `retro`, `prototype`, `wizard`, `kiss` (always-on via hooks), `kiss-audit` |
| **productivity** | `productivity/` | `standing`, `goal`, `where-was-i`, `overwhelm-triage`, `hyperfocus-exit`, `body-double`, `weekly-unstuck`, `mini-reflection`, `nopanic` (ADHD-friendly, see below) |
| **design** | `design/` | `frontend-design`, taste, Vercel guidelines, `cda-design-md` |
| **tools** | `tools/` | Context7 |

**Personal sources (productivity skills).** Several productivity skills read
optional personal sources from `${XDG_CONFIG_HOME:-$HOME/.config}/ai-dev-kit/sources.env`
(`KEY=value`, every key optional, kept outside the repo):

```
JOURNAL_DIR=/path/to/notes/journal   # daily notes (one file per day)
PROJECTS_DIR=/path/to/projects       # one folder per project with an index.md
GH_OWNER=my-org                      # scope gh PR searches
```

Archived skills live under repo-root [`archive/`](../archive/) (not linked by `install.sh`).

Add a new skill under the best-fitting category. Re-run `./install.sh` after
adding a `SKILL.md` — no installer edits required.
