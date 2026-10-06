# Recommended external tools (not vendored)

These are useful alongside ai-dev-kit but are **not** copied into this repo.
Install them separately when you need them.

## ECC — agent harness OS

- Repo: https://github.com/affaan-m/ECC  
- Site: https://ecc.tools  

Full multi-harness agent OS (skills, hooks, security, research workflows). Too
large and overlapping with this kit's foam + kanban + `cda-dev` path to vendor
here. Install from official channels only (see their README warning).

## Superdesign — image to code

- https://superdesign.dev/image-to-code  

SaaS / product workflow for turning designs into code. Keep it as an external
tool. For a local skill-oriented alternative, Taste Skill upstream also ships
`image-to-code-skill` (not vendored in this kit by default).

## awesome-design-md (full catalogue)

- https://github.com/VoltAgent/awesome-design-md  

This kit vendors a **small curated subset** under skill `cda-design-md`. Browse
upstream when you need additional brand `DESIGN.md` files, then copy them into
`skills/design/cda-design-md/brands/` and update `THIRD_PARTY.md`.

## Vercel Web Interface Guidelines (canonical page)

- https://vercel.com/design/guidelines  

The checklist is also available as skill `web-design-guidelines` in this kit
(vendored from vercel-labs/agent-skills). Prefer the skill for agent reviews;
use the live page for human reading / updates.
