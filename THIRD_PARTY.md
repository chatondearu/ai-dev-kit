# Third-party skills and assets

Vendored into this kit for offline install via `install.sh`. Re-sync from upstream
when updating. Pins recorded on 2026-10-07.

| Asset | Upstream | License | Pinned commit |
| ----- | -------- | ------- | ------------- |
| `skills/design/cda-design-taste/` | [Leonxlnx/taste-skill](https://github.com/Leonxlnx/taste-skill) (`skills/taste-skill`) | See upstream repo | `e3c92037548e3e49bea8e6b906c99a8549654e71` |
| `skills/design/cda-design-audit/` | [vercel-labs/agent-skills](https://github.com/vercel-labs/agent-skills) (`skills/web-design-guidelines`) | See upstream repo | `063bee94c3f4df8453406c830b0a7df0f2860278` |
| `skills/design/cda-design-md/brands/*` | [VoltAgent/awesome-design-md](https://github.com/VoltAgent/awesome-design-md) (`design-md/…`) | See upstream repo | `13be5c05c63be24b57581162364167028020f043` |

Kit `name` / folder use the `cda-design-*` prefix for discoverability. On
re-vendor: copy upstream content into the kit folder, then keep the kit
`name:` in frontmatter (`cda-design-taste`, `cda-design-audit`). Kit-authored
wrapper: `cda-design-md`. Bold creative UI (kit-owned): `cda-design-bold`.

## Adapted / inspired (re-authored, not vendored verbatim)

These skills were re-written to fit the kit; the ideas and some wording come from
the upstreams below (all MIT). See [`PHILOSOPHY.md`](PHILOSOPHY.md) for what each
contributed.

| Upstream | License | Adapted into |
| -------- | ------- | ------------ |
| [mattpocock/skills](https://github.com/mattpocock/skills) | MIT | `cda-tdd`, `cda-diagnose`, `cda-grill`, `cda-prototype`, `cda-retro`, `cda-wizard`, `cda-writing`, `cda-codebase`, `cda-architecture`, `cda-standards`; pipeline ideas in `cda-explore`, `cda-triage`, `cda-dev`, `cda-foam` |
| [DietrichGebert/ponytail](https://github.com/DietrichGebert/ponytail) | MIT | `cda-kiss` (+ hooks), `cda-kiss-audit`, and the leanness lens in `cda-review` / `cda-dev` |
| [ayghri/i-have-adhd](https://github.com/ayghri/i-have-adhd) | MIT | `rules/02-adhd-output.md` (reply shape, always-on via CLAUDE.md / Cursor user-rules) |
| [Citability/claude-code-adhd-skills](https://github.com/Citability/claude-code-adhd-skills) | MIT | `cda-standing` (extended to every project) |
| [onion-42/nadja-public-claude-skills](https://github.com/onion-42/nadja-public-claude-skills) | MIT | `cda-goal`, `cda-reflect`, `cda-nopanic`; the anti-loop rule in `cda-kiss` (from `step-back`) |
| [mohitagw15856/pm-claude-skills](https://github.com/mohitagw15856/pm-claude-skills) | MIT | `cda-where`, `cda-overwhelm`, `cda-hyperfocus`, `cda-body-double`, `cda-unstuck` (made evidence-first) |
