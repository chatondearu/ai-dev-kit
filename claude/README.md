# Claude Code configuration

Global instructions for Claude Code are **assembled** from portable `rules/*.md`
(same source as Cursor user rules).

## Files

| File | Role |
| ---- | ---- |
| `CLAUDE.header.md` | Preamble (edit by hand) |
| `../rules/*.md` | Canonical rule fragments (sorted by filename) |
| `CLAUDE.md` | **Generated** — do not edit by hand |

## Regenerate and install

```bash
./scripts/assemble-claude-md.sh
./install.sh --claude
```

`install.sh` runs the assembler automatically when Claude is enabled.

Skills are shared via `~/.claude/skills/` (flattened from `skills/**/SKILL.md`).
Per-project guidance: `AGENTS.md` (skill `project-agents-setup`).
