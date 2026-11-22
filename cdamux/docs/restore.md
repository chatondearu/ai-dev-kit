# Restore, prune, reboot

cdamux keeps three facts about a session:

| Store | Truth for | Survives reboot? |
| ----- | --------- | ---------------- |
| tmux session | what is **live** (ADR-004) | no |
| `$CDAMUX_HOME/sessions/<name>.json` | restore snapshot: cwd, agent, **conversation id** (ADR-009) | yes |
| `$CDAMUX_HOME/agents/<id>.json` | role, status, host, parent (ADR-011) | yes |

`done` / `error` with no live host and **no** registry file cannot resume. They are
unrestorable stubs (`orch ls` `live:false` `host:""`). Idle / working / live hosts
are never pruned.

## Prune

```bash
cdamux state gc --unrestorable          # archive unrestorable done/error now
# same as: cdamux state gc --retention-days 0
cdamux state gc                         # default: age ≥ [state].retention_days (14)
```

Keeps: live hosts, `idle`/`working`/`blocked`/`needs_input`, and registry-dormant
agents (restore by name still works). Never deletes the live global orch.

## Restore conversation

```bash
cdamux restore <name>       # one session; orch name expands to non-done workers
cdamux restore --all        # every recorded non-done session, parents first
cdamux restore --last-orch  # prune unrestorable, then last global orch + workers
```

Resume command comes from the registry `agent_session_id` and `[agents.<id>].resume`
(Cursor default: `cursor-agent --resume={id}`). No id → fresh conversation.

`--all` skips `status=done` (`skipped (agent done; …)`). Restore that name to revive.

## Reboot

tmux start does **not** auto-restore (ADR-009). A user systemd oneshot may call
an **explicit** CLI:

```bash
cdamux restore --last-orch
```

That prunes unrestorable **agent** stubs (not `runs/`), then restores a live
`scope=global` orchestrator if one is already up (ADR-015), else the newest
recorded global orch and its resumable workers. Point `cdamux-restore-dormant`
at `--last-orch` rather than `--all` (`--all` would respawn every idle recorded
worker).

A stub with empty `host` and no `sessions/*.json` cannot come back — prune it.

## Dead agent, live pane

tmux can stay up after the CLI exits (`fish`), or never start (`mise` untrusted).
Reported `working` is not rewritten by a quiet pane (that is only `working ⚠`).
`cdamux list`, Fleet, `worker ls`, `restore`, and `restart` treat a recorded
non-shell session whose `#{pane_current_command}` is a shell / `mise` / `direnv`
as **dead**: status becomes `error`, the parent gets an inbox `error` (nudge when
idle). Host is kept — `cdamux restore <name>` or `restart --force` resumes.
`restart` no longer skips `role=worker` as a shell session. Missing
`pane_current_command` or a shell pane idle under 10s is fail-closed (no reap:
spawn still shows `sh` wrapping the CLI).
