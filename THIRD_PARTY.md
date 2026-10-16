# Third-party skills and assets

Vendored into this kit for offline install via `install.sh`. Re-sync from upstream
when updating. Pins recorded on 2026-10-07.

| Asset | Upstream | License | Pinned commit |
| ----- | -------- | ------- | ------------- |
| `skills/design/design-taste-frontend/` | [Leonxlnx/taste-skill](https://github.com/Leonxlnx/taste-skill) (`skills/taste-skill`) | See upstream repo | `e3c92037548e3e49bea8e6b906c99a8549654e71` |
| `skills/design/web-design-guidelines/` | [vercel-labs/agent-skills](https://github.com/vercel-labs/agent-skills) (`skills/web-design-guidelines`) | See upstream repo | `063bee94c3f4df8453406c830b0a7df0f2860278` |
| `skills/design/cda-design-md/brands/*` | [VoltAgent/awesome-design-md](https://github.com/VoltAgent/awesome-design-md) (`design-md/…`) | See upstream repo | `13be5c05c63be24b57581162364167028020f043` |

Upstream `name` fields are preserved for third-party skills
(`design-taste-frontend`, `web-design-guidelines`) so re-vendoring stays simple.
Kit-authored wrapper: `cda-design-md`.

## Adapted / inspired (re-authored, not vendored verbatim)

These skills were re-written to fit the kit; the ideas and some wording come from
the upstreams below (all MIT). See [`PHILOSOPHY.md`](PHILOSOPHY.md) for what each
contributed.

| Upstream | License | Adapted into |
| -------- | ------- | ------------ |
| [mattpocock/skills](https://github.com/mattpocock/skills) | MIT | `tdd`, `diagnosing-bugs`, `grilling`, `prototype`, `retro`, `wizard`, `writing-for-agents`, `codebase-design`, `improve-codebase-architecture`, `review-standards-spec`; pipeline ideas in `cda-explore`, `cda-triage`, `cda-dev`, `cda-foam` |
| [DietrichGebert/ponytail](https://github.com/DietrichGebert/ponytail) | MIT | `kiss` (+ hooks), `kiss-audit`, and the leanness lens in `code-reviewer` / `cda-dev` |
| [ayghri/i-have-adhd](https://github.com/ayghri/i-have-adhd) | MIT | `rules/02-adhd-output.md` (reply shape, always-on via CLAUDE.md / Cursor user-rules) |
| [Citability/claude-code-adhd-skills](https://github.com/Citability/claude-code-adhd-skills) | MIT | `standing` (extended to every project) |
| [onion-42/nadja-public-claude-skills](https://github.com/onion-42/nadja-public-claude-skills) | MIT | `goal`, `mini-reflection`, `nopanic`; the anti-loop rule in `kiss` (from `step-back`) |
| [mohitagw15856/pm-claude-skills](https://github.com/mohitagw15856/pm-claude-skills) | MIT | `where-was-i`, `overwhelm-triage`, `hyperfocus-exit`, `body-double`, `weekly-unstuck` (made evidence-first) |
