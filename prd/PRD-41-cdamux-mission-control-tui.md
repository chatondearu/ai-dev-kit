# PRD-41 — cdamux mission control dashboard (TUI v2)

- **Status**: approved
- **GitHub**: [#58](https://github.com/chatondearu/ai-dev-kit/issues/58)
- **Parent**: [#40](https://github.com/chatondearu/ai-dev-kit/issues/40) (cdamux epic)
- **Date**: 2026-10-26
- **Tags**: #architecture #tooling #ux

## Problem

v1 shipped a **minimal** ratatui dashboard: flat grouped session list, no preview,
no inline input, spawn via skim after leaving the TUI. Operator expectation (from
agent-manager, epilande, skzv) is a **mission control** surface: one tree of
projects and sessions, live preview, quick send without attach, integrated notes
and review entry, configuration screens, and a rich **new session** flow.

Validated mockup: [`cdamux/prototype/dashboard-ui/`](../cdamux/prototype/dashboard-ui/)
(variant **C+**).

## Goals

- [ ] **Fleet** screen: single left column — discovered **projects** with expandable
  **tmux sessions** underneath; clear visual hierarchy (project vs session) and
  **status icons** (idle / active / needs_input).
- [ ] **Sidecar** on Fleet: tabs **Preview** | **Notes** | **Review** (preview pane
  tail; notes from existing notes config; review launches existing hunk flow).
- [ ] **Preview composer**: type and submit to selected session (same semantics as
  `cdamux send` + optional Enter) without leaving the dashboard.
- [ ] **New session** modal/screen: path from **running project** | **discovered repo**
  | **custom folder**; option **shell only** (no agent CLI); **tmux layout**:
  single pane | split horizontal | split vertical.
- [ ] **App tabs**: Fleet | Projects | Sessions | Agents/CLI | Settings (global config
  and power-user views).
- [ ] **agent-manager parity (phased)**: document and implement high-value keys
  (attach, waiting filter, quick send, review entry, fold/expand tree); defer
  curated logical groups and SSH connection manager unless scoped in a later PRD.
- [ ] Keep **`cdamux --pick`** as a **minimal** popup path (unchanged contract:
  switch-client then exit 0).
- [ ] Update operator docs and `cdamux/docs/research.md` UX row when v2 ships.

## Non-goals

- Replacing tailnet **static web UI** (#45) — complementary, not duplicate every tab
- Full **Agent Deck** / conductor mobile product
- **Teep** file rail or sidecar process
- Curated **agent-manager group trees** with default paths (v2 uses discover +
  project→session tree; optional curated groups → future PRD)
- **install.sh** / default **aitools** Nix enable (ADR-003)
- Rewriting daemon protocol — reuse existing enrichment and send helpers in core

## Users & scenarios

| Persona | Scenario |
| ------- | -------- |
| Maintainer | Open `cdamux` → see projects under `/hdd/dev` → expand → pick session → read preview → send follow-up in composer |
| Multi-session project | Two sessions under same project node; status icons show which needs input |
| New work | `n` → pick discovered repo → cursor agent → split vertical (agent + shell) |
| Shell-only | Quick bash in repo without starting cursor-agent |
| Power user | Sessions tab flat list; Agents tab shows PATH doctor; Settings edits attach_mode |
| Popup | `cdamux --pick` in tmux popup still works without full dashboard |

## Requirements

### Functional

#### Fleet tree

- Merge **discovered projects** (from config roots + discover mode) with **tmux
  sessions** mapped to a project when `session_path` matches a discovered repo
  (longest prefix wins); sessions with no match appear under an **Unassigned**
  (or equivalent) node.
- Project rows: expand/collapse; session rows: selectable for attach, send, preview.
- Visual distinction: project labels ≠ session labels (color and/or weight and
  indentation per mockup).
- Session status from existing `SessionState` (+ daemon when running).

#### Sidecar

- **Preview**: tail of active pane (subprocess or daemon API when available);
  refresh on interval or selection change.
- **Notes**: show note path/content for project tied to selection (`notes` module).
- **Review**: start existing review TUI for selected session (or hand off with
  clear terminal transition).

#### Composer (Preview tab)

- Input line bound to **selected session**; submit → `send_to_session` (+ Enter
  per mockup default).
- `Space` focuses composer when on Fleet Preview (configurable later).

#### New session

- Three path sources as in mockup; **shell only** skips agent resolution.
- Pane layout creates tmux session then optional `split-window` with documented
  rule: **agent (or shell) in primary pane** unless user chooses otherwise in
  settings later.

#### App tabs

- **Projects**: roots, discover settings summary, table of discovered repos.
- **Sessions**: flat tmux list with focus → Fleet selection.
- **Agents/CLI**: configured agents, doctor-style PATH check display.
- **Settings**: edit-safe subset of config (or open path + validate); global UI
  prefs (preview line count, etc.) as TOML keys.

#### Keyboard / AM parity (target set)

| Key | Behavior (v2 target) |
| --- | ---------------------- |
| `j`/`k`, arrows | Tree navigation |
| `Enter` | Attach selected session |
| `n` | New session flow |
| `Space` | Focus composer / quick send when implemented |
| `w` | Toggle waiting-for-input filter |
| `F` / fold control | Expand/collapse all project nodes |
| `g` | Cycle `group_by` where it affects tree ordering (document vs AM `g`) |
| `Ctrl+R` | Open review from Fleet |
| `q` | Quit dashboard |

Not all keys required in phase 1; see implementation plan phases.

### Non-functional

- English user-facing strings (i18n later).
- Terminal restore on quit/error (existing `TerminalGuard` pattern).
- No new telemetry crates.
- Performance: usable with ~50 projects and ~30 sessions on maintainer machine.

## Related project memory

- PRD-39 (v1 shipped): [[PRD-39-cdamux|PRD-39]]
- ADRs: [[../foam/decisions/ADR-001-host-tools-vs-kit|ADR-001]], [[../foam/decisions/ADR-003-cda-aitools|ADR-003]], [[../foam/decisions/ADR-004-cdamux|ADR-004]]
- Research: [`cdamux/docs/research.md`](../cdamux/docs/research.md)
- Prototype: [`cdamux/prototype/dashboard-ui/README.md`](../cdamux/prototype/dashboard-ui/README.md)

## Open questions

- Preview source when **cdamuxd** is off: direct `tmux capture-pane` only?
- Multi-line composer: Shift+Enter vs external editor for long prompts?
- **Move session** (`m`) between project nodes — metadata only or tmux rename?
- Dedicated tmux socket `-L cdamux` still deferred?

## Success metrics

- Maintainer daily-drives Fleet for attach + send + spawn for one week.
- Dogfood checklist extended with v2 scenarios; no regression on `--pick`.
- All phases in PLAN-41 complete with `cargo test` green.
