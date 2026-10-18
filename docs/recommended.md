# Recommended external tools (not vendored)

These are useful alongside ai-dev-kit but are **not** copied into this repo.
Install them separately when you need them. The kit does **not** package or
update host apps — run `./scripts/doctor.sh` to see what is on your `PATH`.

## Host agent CLIs (for orch / headless)

Install via each product’s official channel (or your NixOS / home-manager).
`orch.sh` and `headless.sh` auto-detect, in order: `agent`, `cursor-agent`,
`claude`, `opencode` (override with `ORCH_AGENT`).

| CLI | Notes |
| --- | ----- |
| Cursor Agent | Cursor subscription; headless often needs trust/auto-approve flags |
| Claude Code (`claude`) | Anthropic subscription / CLI |
| OpenCode (`opencode`) | Alternative coding CLI |
| `gh`, `git` | Required for Kanban / CI modes of `orch.sh` |
| `flock`, `jq` | Required for `orch-auto.sh` (Ready poller) |

Optional: `bwrap` for `--sandbox`, `llama-swap` for local models (point
`ORCH_AGENT` at a harness that talks to your local endpoint).

## AFK orch-auto (systemd, opt-in)

Poll Ready Kanban cards and run `orch.sh --mode ready-pickup` without sitting in
the IDE. Templates live in
`skills/core/cda-dev/templates/systemd/` (not enabled by `install.sh`).

```bash
SKILL=~/.cursor/skills/cda-dev
mkdir -p ~/.config/systemd/user
cp "$SKILL/templates/systemd/cda-orch-auto."{service,timer} ~/.config/systemd/user/
cp "$SKILL/templates/systemd/cda-orch-auto.env.example" ~/.config/cda-orch-auto.env
# edit ORCH_REPO (and PATH / ORCH_AGENT) in ~/.config/cda-orch-auto.env
systemctl --user daemon-reload
systemctl --user enable --now cda-orch-auto.timer   # AFK ON
systemctl --user stop cda-orch-auto.timer            # free this machine
```

Prefer a **dedicated host** (repo clone + `gh auth` + agent CLI) when you want
the laptop free for other work. Operator guide:
[`doc/orchestration.md`](../doc/orchestration.md). Fast-lane (brief without
PRD/plan): foam [ADR-002](../foam/decisions/ADR-002-orch-auto-and-fast-lane.md);
headless stays a separate one-prompt tool
([ADR-001](../foam/decisions/ADR-001-host-tools-vs-kit.md)).

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

## Eyes for design agents (screenshots)

Agents need to **see** rendered pages for `cda-design-loop`. Prefer, in order:

1. Playwright MCP or Cursor browser tools already available in the session
2. Project script `scripts/cda-design-shot.sh` (from `seed-design.sh`) if Playwright is already installed locally
3. Otherwise the loop reports `SKIP_VISUAL` and still runs textual checks

Do not auto-install Playwright from the kit. Superdesign remains optional for image→code (see above).
