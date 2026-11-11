# PLAN-242 — Wire real agent statuses and client status hooks

- **Status**: draft
- **PRD**: [[../prd/PRD-239-orchestrator-session|PRD-239]]
- **GitHub**: [#242](https://github.com/chatondearu/ai-dev-kit/issues/242)
- **Blocked by**: #241
- **Date**: 2026-11-11

## Summary

Slice 1 completion of PRD-239: the seven-status model with precedence and
stickiness, opt-in client status hooks for all four products, and Fleet reading
agent status from `CDAMUX_HOME` so interactive sessions no longer depend on
`@cdamux_state`.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-011-cdamux-unified-state-model|ADR-011]] | §3 status set, precedence, stickiness, `working ⚠`; writes only via `cdamux state` |
| [[../foam/decisions/ADR-007-cdamux-session-hierarchy|ADR-007]] | `--cdamux-hooks` opt-in; plain `install.sh` never wires; hooks fail-open, local, fast |
| [[../foam/decisions/ADR-001-host-tools-vs-kit|ADR-001]] | Do not install client CLIs; hooks no-op without `cdamux` on PATH |
| [[../foam/decisions/ADR-002-orch-auto-and-fast-lane|ADR-002]] | Do not enable hooks from `install.sh` by default |
| [[../foam/decisions/ADR-009-cdamux-session-restore-registry|ADR-009]] | `dormant` remains a reader-computed status when no live host |

Do not violate accepted ADRs without superseding them.

## Scope

### In scope

- Status set: `working` / `idle` / `needs_input` / `blocked` / `done` / `error` / `dormant`
- Precedence: reported wins; heuristics fallback; `working ⚠` after 10 min quiet pane (configurable)
- Stickiness: `done` / `error` until next prompt submit
- Status hooks for claude, cursor, opencode, codex via `install.sh --cdamux-hooks` and `cda.aitools.cdamux.hooks.enable`
- Cursor mapping: `sessionStart`/`sessionEnd` → `idle`; `beforeSubmitPrompt` → `working` when fired; document `-p` limitation
- Fleet reads status from `CDAMUX_HOME` agent state

### Out of scope

- State CLI / agent id foundation (#241 — blocker)
- Messages, worker spawn, orchestrator skill (slice 2)
- orch.sh hard cut / run migration (slice 3); absorbs #236 later
- Fleet orchestrator tree UI, restore GC (slice 4)
- Fix #237 (kiss `additional_context`) unless required for status hooks

## Technical approach

Reuse the existing cdamux-hooks wire path: add status adapters that call
`cdamux state set` for the agent id in the environment. Fleet status derivation
prefers the agent file under `CDAMUX_HOME`, with pane heuristics only when no
reported status exists. Keep `@cdamux_state` readable as a transitional fallback
if cheap, but interactive hooked sessions must not need it.

## Tasks

- [ ] **Status model**: store and read the seven statuses with precedence and stickiness rules from ADR-011 §3 (acceptance: unit tests for winner, sticky done/error, dormant when host missing)
- [ ] **`working ⚠`**: when reported `working` and pane quiet longer than config default 10 min, display warning without changing stored status (acceptance: test with injectable clock / activity stamp)
- [ ] **Hooks**: prompt-submit → `working`, turn-end → `idle`, permission/notification → `needs_input` where the client exposes the event; wire through existing `--cdamux-hooks` / Nix enable; fail-open < 50 ms typical; no-op without `cdamux` (acceptance: hook unit tests + wire test on temp config dirs)
- [ ] **Cursor specifics**: map `sessionStart`/`sessionEnd` → `idle`, `beforeSubmitPrompt` → `working`; document that `cursor-agent -p` may omit `beforeSubmitPrompt` (acceptance: docs + hook tests with fixture payloads)
- [ ] **Fleet**: prefer `CDAMUX_HOME` agent status over `@cdamux_state` for display (acceptance: Fleet/list test with temp home + mocked agent file)
- [ ] **Verify**: `nix develop -c bash scripts/verify.sh` green; never run install against live `HOME` / `CLAUDE_CONFIG_DIR` / `CURSOR_HOME`

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Cursor `-p` skips `beforeSubmitPrompt` | Rely on sessionStart/End; document; optional interactive check later |
| Hooks write without agent id | No-op when `CDAMUX_AGENT_ID` / `@cdamux_id` missing |
| Breaking orch `@cdamux_state` workers | Keep heuristic/`@cdamux_state` fallback until slice 3 |

## Test plan

Seams: (1) status precedence/stickiness, (2) hook → `cdamux state set`, (3) wire/unwire on temp configs, (4) Fleet/list status source.

- [ ] Precedence + stickiness + `working ⚠` unit tests
- [ ] Per-client hook payload fixtures → expected status writes
- [ ] Wire/unwire on temp `HOME` / config dirs (never live)
- [ ] Fleet/list reads agent file from temp `CDAMUX_HOME`
- [ ] `nix develop -c bash scripts/verify.sh`

## Verification

- [ ] Lint / test / build per project conventions
- [ ] Update foam / ADR only if a new decision was made
