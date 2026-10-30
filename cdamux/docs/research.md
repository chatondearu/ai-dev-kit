# cdamux — research & competitor matrix

Captured 2026-10-26 from operator dogfood (skzv/ccmux 0.6.4, agent-manager,
epilande/ccmux docs, pickers). **Orca** section added 2026-10-29. Update this file
when evaluating new tools.

## Operator goals (why cdamux exists)

1. **Orchestrator model** — the tool spawns and supervises tmux sessions (skzv/ccmux),
   not only jump to panes started by hand (epilande picker-first).
2. **Multi-root and nested repos** — e.g. `/hdd/dev/org/repo`, plus paths outside
   one `projects.root` (NixOS configs under `/home/chaton/etc/nixos`).
3. **Dynamic session grouping** — `groupBy` project/cwd/session like epilande, not
   only manual group trees (agent-manager).
4. **Popup-friendly picker** — choose session → attach/switch → **process exits**
   (`tmux display-popup -E`).
5. **New session** — explicit agent + directory (tree browse + custom path).
6. **Remote** — tailnet + Mosh/SSH; optional browser UI later.
7. **Keep** — quick prompt, review-to-agent, notes/markdown (prioritize by phase).

## Tool comparison matrix

| Attout | skzv/ccmux | ccmux epilande | agent-manager | Agent Deck | MuxPilot | taimux | tmux-agent (`ta`) | Teep |
| ------ | ---------- | -------------- | ------------- | ---------- | -------- | ------ | ----------------- | ---- |
| **Spawns tmux sessions** | Yes | Partial | Yes (`am_*` on `agentmgr`) | Yes | No | No | No | No |
| **Daemon / live state** | `ccmuxd` | Daemon | Poll tmux | Registry | Scan tmux | Scan panes | Scan | No |
| **Multi-agent CLIs** | Many | Many | 12 built-in | Many | Detect | Detect | Claude/Codex | N/A |
| **Projects on disk** | 1 root, **1 level** discover | 1 root | Per-group default path | Per-session path | zoxide+git+dirs | CWD column | Pane cwd | N/A |
| **Multi-root** | `--projects` override only | Override | Manual groups | Add per path | Many sources | SSH hosts | N/A | N/A |
| **groupBy dynamic** | No | **project/cwd/session/window** | No (logical tree) | Named groups | By target type | By pane | By pane | N/A |
| **Picker quits on select** | No (TUI returns) | Yes (`persistent`) | Manager stays | Varies | **Yes** | **Yes** | **Yes** | N/A |
| **Reply without attach** | `send` | send/preview | **space** | send | No | No | No | N/A |
| **Diff review → agent** | hunk review | hunk | **ctrl+r** | Yes | No | No | No | Diff view |
| **Worktrees** | Yes (spawn + list) | Yes | Rich | Yes | Switch | No | **ta switch worktree** | No |
| **Tailnet / remote** | ccmuxd HTTP | Daemon | SSH `C` | Conductor | No | SSH panes | No | No |
| **Mobile** | Moshi + API | Moshi | SSH attach | Web conductor | Mosh popup | Mosh | Mosh | No |
| **Notes / MD** | Glamour in TUI | Yes | No | No | No | No | No | **Best-in-class** |
| **Web UI** | Roadmap | ? | Not public | Yes | No | No | No | No |
| **Licence** | FSL-1.1-MIT | MIT | Apache-2.0 | MIT | (check repo) | MIT/Apache | (check repo) | MIT/Apache |
| **Stack** | Go | Go | Go | Go | **Rust** | **Rust** | **Rust** | **Rust** |

## Per-tool notes

### skzv/ccmux (Nix 0.6.4, dogfood default in `aitools/`)

**Liked**

- ccmux **creates** tmux sessions; dashboard, Projects, Conversations, Notes.
- Tailscale peer discovery, `ccmux attach`, per-project `.ccmux/agent`.
- Moshi integration path documented in kit.

**Limits**

- `internal/project.Discover`: **only immediate children** of `projects.root`.
- Single `projects.root` in config (no `roots[]`).
- TUI inside tmux popup: `switch-client` then **returns to dashboard** (popup stays).
- No `ccmux config set` / epilande `groupBy` (different product line).

**Config (kit, historical)**: was `aitools/templates/ccmux/`; now `aitools/templates/cdamux/config.toml`.

### ccmux epilande (ccmux.dev, Homebrew core — kit removes in favor of skzv)

**Liked**

- Session picker with `groupBy`: `project | cwd | session | window | none`.
- `persistent: false` → close after switch.
- `ccmux spawn`, preview panel, worktrees, `send`, hunk review.
- `ccmux.json` preferences (`showPreview`, `previewWidth`, etc.).

**Limits**

- Often assumed **existing tmux**; less “greenfield project from UI” than skzv.
- Kit migrated to in-tree cdamux (`aitools/scripts/cdamux-align-mac.sh`).

### agent-manager (agent-manager.dev)

**Liked**

- **`n`**: name, tool, model, effort, **directory**, worktree, group, prompt.
- **`space`**: quick prompt; spawn in group with `tab` / `ctrl+l` / `ctrl+t`.
- Nested **logical groups** (`backend/api/auth`) with default path per group.
- Review mode, revive/fork, SSH connections, private tmux `agentmgr`.
- Apache-2.0.

**Limits**

- **No** epilande-style `groupBy` on live sessions — groups are curated, not views.
- Does not auto-scan `/hdd/dev/**` for git repos; map disks via group `default path`.
- Sessions on separate tmux server — not mixed with personal tmux (by design).

**Keys (reference)**: `n` new, `g` group, `A` fullscreen attach, `enter` focus in place,
`w` waiting filter, `F` fold all, `m` move, `ctrl+r` review.

### Agent Deck (asheshgoplani/agent-deck)

Mission control TUI: groups, fork, worktrees, cost, MCP per session, conductor/mobile angle.
Heavier than needed for v0; good reference for registry model and web.

### MuxPilot (Rust)

Agent-aware tmux picker: sessions, tmuxinator, zoxide, git repos, **expand session → windows**.
Ideal **pick UX** reference; no spawn orchestration.

### taimux (Rust)

Lists **panes running agents**, jump, Claude hooks, restart/resurrect. Picker-only;
excellent for “find agent pane” without owning spawn.

### tmux-agent `ta` (Rust)

`ta switch` / agent / worktree with preview; tmux keybinds. Composable with cdamux pick.

### Teep (Rust)

Sidecar **viewer** (files, diff, markdown/mermaid) — complement, not mux. Phase 2+ idea.

### Orca (onorca.dev)

[Orca](https://www.onorca.dev/) is a **desktop Agent Development Environment** (MIT,
[stablyai/orca](https://github.com/stablyai/orca)): worktrees, parallel agent terminals,
embedded browser/editor, mobile companion, and an **experimental orchestration** layer
(Run / Task / Dispatch, `worker_done`, decision gates). It overlaps cdamux on **fleet +
worktrees + send + review** and overlaps the kit on **parallel workers + completion tracking**,
but Orca owns the full GUI and supervised inbox; we keep **tmux + bash orch + foam**.

**Kit mapping (detailed):** [`doc/orca-mapping.md`](../../doc/orca-mapping.md).

#### Orca vs cdamux vs kit (selected rows)

| Attout | Orca | cdamux | Kit (`cda-dev` / `cda-graph`) |
| ------ | ---- | ------ | ------------------------------ |
| **Spawns agent sessions** | Yes (native + CLI) | Yes (`spawn`, tmux) | `orch.sh` workers (worktrees, not tmux API) |
| **Worktrees** | Core UX + CLI | Opt-in spawn modal; `worktree list` | `.orch/worktrees/` via orch |
| **Structured multi-agent** | Orchestration inbox | Read-only Orch graph sidecar | `current.json`, integrator, `--graph-plan` |
| **GitHub / Linear UI** | In-app | — | `cda-kanban`, `gh` |
| **Review → agent** | Inline diff comments | Hunk review TUI | PR / `cda-review` |
| **Remote** | SSH worktrees, federated workers | Tailnet HTTP + pane send | — |
| **Mobile** | iOS / Android app | Web UI (tailnet) | — |
| **Agent-driven CLI** | `orca … --json` | Partial (`cdamuxd` API) | `orch.sh`, `headless.sh` |
| **Licence** | MIT | (extract-time dual licence) | MIT kit |

**Liked (reference)**

- One app for parallel agents + worktrees + diff review without branch juggling.
- Supervised worker contract (`worker_done`, dispatch IDs) for coordinator loops.
- GitHub/Linear → worktree from task (operator flow we do via issues + orch pickup).
- CLI so agents drive the environment (`orca skills get orchestration --full`).

**Limits (vs our goals)**

- Not tmux-native — different from maintainer popup/`M-p` + personal tmux mix.
- Orchestration state lives in Orca runtime, not repo-local JSON + foam.
- Heavy desktop; we optimize for Nix/tmux/headless orch on a always-on host.
- Multi-root discover and `/hdd/dev/**` layout are cdamux config concerns Orca solves differently (workspace folders).

**Do not confuse**

- Orca **orchestration** ≠ kit **graph** file alone: we persist `.orch/graph/current.json`
  but do not implement Delivery ack or `dispatchId` completion authority.
- cdamux **Orch graph** tab is **observability** of orch runs, not Orca’s Task scheduler.

## UX decisions to steal (cdamux v0 → v1)

| Phase | Feature | Source |
| ----- | ------- | ------ |
| v0 | `roots[]`, discover `git_only` / `depth` | skzv gap + MuxPilot dirs |
| v0 | `group_by` enum on session list | epilande |
| v0 | `--pick` exit after `switch-client` | epilande + MuxPilot popup |
| v0 | Spawn form: agent + path tree + custom | agent-manager `n` |
| v0.1 | `send` without attach | agent-manager space / ccmux send |
| v0.2 | Tailnet HTTP + static web | skzv ccmuxd API + Agent Deck |
| v1 | Hunk review → agent | agent-manager |
| v1 | Notes markdown per project | skzv Notes + Teep render |
| v2 | Fleet tree + preview composer + spawn modal | PRD-41 / prototype C+ |
| v2 | `w` waiting filter, `F` fold all, `g` group_by cycle | agent-manager parity (see `?` help: AM `g` ≠ cdamux `g`) |
| v2 | App tabs (Projects, Sessions, Agents, Settings) | mockup top nav |
| future | Issue # → spawn hint / Fleet link | Orca GitHub tasks (see `doc/orca-mapping.md`) |
| future | Worker completion artifact enforcement | Orca `worker_done` (kit: `worker-result.json`) |

## tmux integration (kit)

From `aitools/templates/tmux.conf`:

```tmux
bind-key -n M-p display-popup -E -w 80% -h 75% "ccmux"
```

Target for cdamux:

```tmux
bind-key -n M-p display-popup -E -w 80% -h 75% "cdamux"
```

## skzv vs epilande CLI (do not confuse)

| | skzv 0.6.x | epilande (docs) |
| - | ---------- | --------------- |
| Config | `config.toml` | `ccmux.json` + `config set` |
| Entry | Full TUI tabs | `ccmux picker`, `persistent` |
| Project discover | 1 level | Picker groups sessions |

## References

- skzv README (bundled in nix store package): project discover one-level comment.
- [agent-manager sessions](https://agent-manager.dev/docs/sessions/)
- [ccmux configuration (epilande)](https://ccmux.dev/docs/configuration/)
- [MuxPilot](https://github.com/muxpilot/muxpilot)
- [taimux](https://github.com/pdecat/taimux)
- [Orca](https://www.onorca.dev/) — [orchestration docs](https://www.onorca.dev/docs/cli/orchestration)
- [`doc/orca-mapping.md`](../../doc/orca-mapping.md) (kit crosswalk)
- ADR-003, ADR-004, PRD-39
