# Foam board

#kanban #foam

Execution status for this repo. **Source of truth** (ADR-016). GitHub
Projects is not. GitHub Issues are optional (`github_issue` in frontmatter).

One markdown file per card. Readers glob `foam/board/*.md` and skip
`README.md` plus names starting with `_`. Files without required frontmatter
are not cards. Do not add a hand-edited `index.md` here — it becomes a merge
conflict.

Status flow: **Backlog → Ready → In progress → In review → Done**.

## Frontmatter

Copy [_template.md](_template.md). Required: `status`, `priority`, `updated`.
Optional: `title`, `github_issue`, `blocked_by`.

| Field | Values |
| ----- | ------ |
| `status` | `backlog` \| `ready` \| `in-progress` \| `in-review` \| `done` |
| `priority` | `P0` \| `P1` \| `P2` |
| `updated` | `YYYY-MM-DD` |
| `github_issue` | integer; omit if the card has no GitHub issue |
| `blocked_by` | list of issue numbers and/or slugs; omit if none |

Filename: `{slug}.md`. When `github_issue` is set, prefer `{N}-{slug}.md`.

## Related

- [[../decisions/ADR-016-foam-first-kanban|ADR-016]]
- Fleet browse: epic [#265](https://github.com/chatondearu/ai-dev-kit/issues/265) (UI only; not this board)
- Ready oracle + status: `cda-kanban` `foam-board.sh` (`list-ready`, `claim`, `set-status`)
- Optional GH helper: `cda-kanban` `gh-board.sh` (opt-in issues only)
- One-shot GH → cards: `cda-foam` `import-board.sh` (not a loop)
