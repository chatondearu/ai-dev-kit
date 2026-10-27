# Orchestration — how to run

Skill install path (after `./install.sh`):

```bash
SKILL=~/.cursor/skills/cda-dev   # or Claude / opencode skills dir
```

Host readiness (checks only, never installs):

```bash
./scripts/doctor.sh
```

## Graph orchestration (`cda-graph`)

By default `orch.sh` writes a live run graph to `<repo>/.orch/graph/current.json`
(workers, verify nodes, integrator). **cdamux** Fleet → sidecar tab **Orch graph**
polls this file when the selected session’s cwd is under that repo.

```bash
ORCH_GRAPH_VERIFY=1 bash "$SKILL/scripts/orch.sh" --mode ready-pickup --issue 12
bash ~/.cursor/skills/cda-graph/scripts/foam-graph-export.sh --repo . --merge-orch
```

Intra-issue fan-out:

```bash
bash "$SKILL/scripts/orch.sh" --mode ready-pickup --issue 42 \
  --graph-plan ./skills/core/cda-graph/templates/graph-plan.example.json
```

**cdamuxd** tailnet dashboard: select a session → **Orch graph** tab (polls
`GET /v1/sessions/{name}/orch-graph`). TUI Fleet uses the same JSON file.

See skill `cda-graph` and [[ADR-005](../foam/decisions/ADR-005-cda-graph-orchestration.md)].

## Manual ready-pickup

```bash
bash "$SKILL/scripts/orch.sh" --mode ready-pickup --issue 12 --issue 15
bash "$SKILL/scripts/orch.sh" --mode ready-pickup --issue 12 --dry-run
bash "$SKILL/scripts/orch.sh" --mode pr-fix --pr 88
```

Requires `gh`, a detected agent CLI, and (for fast-lane) an agent brief on the
issue when no `plans/PLAN-<id>-*` exists.

## AFK poller (`orch-auto`)

Discovers open Issues in board Status **Ready**, skips
`triage:ready-for-human` and issues that already have an open PR on
`feat/<id>-issue-<id>`, then runs `orch.sh --mode ready-pickup`.

```bash
bash "$SKILL/scripts/orch-auto.sh" --repo /path/to/repo --once --dry-run
bash "$SKILL/scripts/orch-auto.sh" --repo /path/to/repo --once
```

Board listing uses `cda-kanban` `gh-board.sh list-ready`. Override path with
`ORCH_GH_BOARD` if needed (tests / custom layout).

One run at a time: flock on `<repo>/.orch/auto.lock`. Logs under
`<repo>/.orch/auto/`.

### Systemd (opt-in)

Templates: `skills/core/cda-dev/templates/systemd/`. **Not** enabled by
`install.sh`.

```bash
mkdir -p ~/.config/systemd/user
cp "$SKILL/templates/systemd/cda-orch-auto."{service,timer} ~/.config/systemd/user/
cp "$SKILL/templates/systemd/cda-orch-auto.env.example" ~/.config/cda-orch-auto.env
# Edit ORCH_REPO (and PATH / ORCH_AGENT) in the env file
systemctl --user daemon-reload
systemctl --user enable --now cda-orch-auto.timer   # AFK ON
systemctl --user stop cda-orch-auto.timer            # free this machine
```

Prefer a **dedicated host** (clone + `gh auth` + agent CLI) when the laptop
should stay free for interactive work.

## Headless (no Kanban)

```bash
bash "$SKILL/scripts/headless.sh" --prompt "fix the flaky auth test" --dry-run
bash "$SKILL/scripts/headless.sh" --prompt-file ./task.md --max-iter 5
```

## Triage handoff

1. Groom with skill `cda-triage` until `ready-for-agent` + Status Ready + brief.
2. Either run orch manually, or enable orch-auto / the timer.
3. Review the PR; merge yourself.
