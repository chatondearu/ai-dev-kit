# Configuration

## Host CLIs

| Tool | Used by |
| ---- | ------- |
| Agent CLI (`agent`, `cursor-agent`, `claude`, `opencode`) | `orch.sh`, `headless.sh` |
| `gh`, `git` | Kanban / CI modes of orch |
| `jq`, `flock` | `orch-auto.sh`, `orch.sh` (ready-pickup / pr-fix) |
| `bwrap` (required for `--sandbox`) | `--sandbox` exits if missing (no unsandboxed fallback) |

See [`docs/recommended.md`](../docs/recommended.md). The kit never installs
these (`./scripts/doctor.sh` only checks `PATH`).

## Environment variables

| Variable | Default | Meaning |
| -------- | ------- | ------- |
| `ORCH_AGENT` | auto-detect | Override agent invoke command |
| `ORCH_MAX_PARALLEL` | `3` (`1` for orch-auto default) | Parallel ready-pickup workers |
| `ORCH_MAX_ITER` | `10` | Verify-loop retries per worker |
| `ORCH_PROGRESS` | `auto` | Set `0` to disable TTY dashboard |
| `ORCH_SANDBOX` | off | `1` / `--sandbox` → bubblewrap required |
| `ORCH_ALLOW_NO_CHECKS` | `0` | `1` → empty PR checks count as green |
| `ORCH_COPY_ENV` | `0` | `1` → copy `.env` into worktrees |
| `ORCH_VERIFY` | unset | Override verify command (allowlisted under pr-fix) |
| `ORCH_FRONTIER` | `auto` | `0` disables Blocked-by gating |
| `ORCH_FORCE_CLEAN` | off | Remove worktrees after success |
| `ORCH_REPO` | cwd / `--repo` | Target git tree (required in systemd env) |
| `ORCH_GH_BOARD` | discovered | Path to `gh-board.sh` for orch-auto |
| `ORCH_AUTO_ONCE` | off | Force one orch-auto cycle |
| `CDA_DESIGN_LOOP` | off | Opt-in design loop for UI diffs |

## On-disk state (under the target repo)

| Path | Purpose |
| ---- | ------- |
| `.orch/worktrees/` | Per-issue worktrees |
| `.orch/run/` | Worker exit codes / progress |
| `.orch/headless/` | Headless prompt copy |
| `.orch/auto.lock` | orch-auto mutex |
| `.orch/orch.lock` | orch ready-pickup / pr-fix mutex |
| `.orch/worktrees/` | Per-issue worktrees (prune with `orch-cleanup.sh`) |
| `.orch/auto/*.log` | orch-auto run logs |

## Systemd env file

Example: `skills/core/cda-dev/templates/systemd/cda-orch-auto.env.example` →
`~/.config/cda-orch-auto.env`. At minimum set `ORCH_REPO` to an absolute path.
Ensure agent CLIs and `gh` are on `PATH` for the user systemd session.
