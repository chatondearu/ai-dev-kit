# PLAN-41 — cdamux mission control dashboard (TUI v2)

- **Status**: in-progress
- **PRD**: [[../prd/PRD-41-cdamux-mission-control-tui|PRD-41]]
- **GitHub**: [#58](https://github.com/chatondearu/ai-dev-kit/issues/58)
- **Prototype**: [[../cdamux/prototype/dashboard-ui/README|dashboard-ui prototype]]
- **Date**: 2026-10-26

## Summary

Replace the v1 flat session list TUI with the validated **C+** mockup: Fleet tree,
sidecar, composer, spawn modal, app tabs, and phased agent-manager keyboard parity.
Work is split into **six phases**; each phase should merge as a vertical slice with
tests where behavior is non-trivial.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-004-cdamux|ADR-004]] | tmux source of truth; optional daemon |
| [[../foam/decisions/ADR-003-cda-aitools|ADR-003]] | No aitools package by default |
| [[../foam/decisions/ADR-001-host-tools-vs-kit|ADR-001]] | Optional host tool |

## Scope

### In scope

All PRD-41 goals, delivered across phases 1–6 below.

### Out of scope

- Web UI parity with every TUI tab
- Epic #56 extract (separate track)
- Curated AM-style group CRUD

## Technical approach

- Extend `cdamux-cli` TUI module(s); add a **tree model** in `cdamux-core` that joins
  `discover_projects` output with `grouped_for_list` / session paths.
- **App state machine**: top-level `AppTab` + Fleet `SideTab` + selection +
  `expanded_projects` set.
- Reuse `send_to_session`, `spawn_tmux_session`, notes helpers, review entry from CLI.
- Preview: `capture_pane_text` (core/tmux) with line limit from config; optional
  daemon refresh when socket present.
- Spawn overlay: ratatui **popup** or alternate full-screen form; pane splits via
  tmux commands after session create.
- `--pick` remains separate code path (skim list only).

## Phases

### Phase 1 — Fleet tree (foundation)

**Outcome:** Default `cdamux` shows project→session tree with expand/collapse and
status styling; attach on Enter; `q` quit; `--pick` unchanged.

- [ ] Core: build `ProjectSessionTree` (projects, sessions per project, unassigned bucket) from discover + tmux list
- [ ] Core: unit tests for path→project matching (nested repos, two roots)
- [ ] TUI: render tree with project vs session styles and status icons (idle/active/needs_input)
- [ ] TUI: keys — toggle expand, j/k navigation, Enter attach, preserve terminal guard
- [ ] Docs: one paragraph in `architecture.md` + dogfood scenario for tree

**Acceptance:** `cargo test`; manual: expand project, attach session, `--pick` still exits 0 in popup.

---

### Phase 2 — Preview sidecar + composer

**Outcome:** Fleet split view; Preview tab shows pane tail; composer sends to selection.

- [ ] TUI: layout — tree column + sidecar column (ratatui constraints)
- [ ] Preview: fetch pane text on selection change (line limit config key, default 80)
- [ ] Sidecar tabs shell: Preview | Notes | Review (latter two stub → phase 4)
- [ ] Composer on Preview: submit calls existing send API with Enter
- [ ] Key: Space focuses composer when Preview active
- [ ] Test: send payload builder reused (no duplicate escaping logic)

**Acceptance:** Send message visible in target tmux pane; preview updates after send.

---

### Phase 3 — New session modal

**Outcome:** `n` opens in-TUI spawn: running / discovered / custom path; shell-only; pane layout.

- [ ] TUI: modal state — path mode enum, agent select, shell-only flag, layout enum
- [ ] Wire discovered list from core discover; running list from active sessions/projects
- [ ] Spawn: integrate shell-only (bash in pane, no agent command)
- [ ] Spawn: after create, optional `split-window -h`/`-v` with documented primary pane rule
- [ ] On success: refresh tree, select new session

**Acceptance:** Three path modes work; shell-only creates bash session; split creates two panes.

---

### Phase 4 — Notes & Review sidecar

**Outcome:** Notes tab shows project note; Review tab launches existing review TUI.

- [ ] Notes tab: resolve note path from selected project/session; render plain or `--render` markdown subset in sidecar (height-limited)
- [ ] Review tab / `Ctrl+R`: exit-friendly handoff to `review` flow for selected session
- [ ] Dogfood: document keys in `dogfood.md`

**Acceptance:** Notes visible for ai-dev-kit path; review opens on sample repo session.

---

### Phase 5 — App tabs (Projects, Sessions, Agents, Settings)

**Outcome:** Top-level tab bar; non-Fleet screens usable for config inspection.

- [ ] TUI: `AppTab` navigation (keys or tab bar focus)
- [ ] Projects: list roots + discovered projects (read-only table)
- [ ] Sessions: flat list; select → jump to Fleet with selection
- [ ] Agents: list configured ids + doctor PATH result (cached per dashboard open)
- [ ] Settings: show/edit selected keys (attach_mode, group_by default, preview lines, daemon flags) with save to config file

**Acceptance:** Can change `attach_mode` from Settings and see effect on attach.

---

### Phase 6 — AM parity polish + docs

**Outcome:** Waiting filter, fold all, group_by cycle, help bar; PRD-39 status note.

- [x] `w` waiting filter on tree
- [x] `F` expand all / fold all project nodes
- [x] `g` cycle group_by when it affects tree ordering (document difference from AM)
- [ ] Optional: `m` move/rename session (stretch — deferred)
- [x] Update `research.md` UX table; `prompts/next-agent.md` → v2 maintenance
- [x] PRD-41 → `shipped`; close #58 (after stack merge + dogfood sign-off)

**Acceptance:** Keyboard help matches implemented keys; full `cargo test`; maintainer sign-off comment on #58.

## Test plan

- Core tree matching: unit tests (phase 1)
- Send/composer: reuse existing core tests + one integration test with mocked tmux if present
- Manual dogfood: extended checklist v2 section

## Verification

```bash
cd cdamux && cargo build && cargo test
cargo run -p cdamux-cli -- doctor
# manual: cdamux → Fleet → composer → spawn modal → tabs
```

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| TUI complexity explosion | Strict phases; no phase 5 until phase 2 stable |
| Preview latency | Limit lines; debounce capture |
| Modal + nested tmux | Test popup bind separately |
