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
