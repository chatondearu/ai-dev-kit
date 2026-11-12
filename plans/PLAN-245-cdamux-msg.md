# PLAN-245 — Add cdamux msg inbox send/read and delivery

- **Status**: done
- **PRD**: [[../prd/PRD-239-orchestrator-session|PRD-239]]
- **GitHub**: [#245](https://github.com/chatondearu/ai-dev-kit/issues/245)
- **Date**: 2026-11-12

## Summary

Slice 2 message transport: inbox under `CDAMUX_HOME`, `cdamux msg send|read`,
delivery to workers (pane paste) and orchestrators (idle nudge + turn-end hooks),
and `ask`/`answer` status effects per ADR-011 §4.

## Constraints (from ADRs)

| ADR | Constraint |
| --- | ---------- |
| [[../foam/decisions/ADR-011-cdamux-unified-state-model|ADR-011]] | §4: inbox JSONL ≤ 2 KB, kinds, ack cursor, delivery modes, ask→blocked |
| [[../foam/decisions/ADR-001-host-tools-vs-kit|ADR-001]] | No-op without `cdamux` on PATH; do not install host CLIs |
| [[../foam/decisions/ADR-007-cdamux-session-hierarchy|ADR-007]] | Hooks opt-in via `--cdamux-hooks`; fail-open, local, fast |
| [[../foam/decisions/ADR-009-cdamux-session-restore-registry|ADR-009]] | Inbox paths under `CDAMUX_HOME`; ids are agent ids not tmux names |

Do not violate accepted ADRs without superseding them.

## Scope

### In scope

- `cdamux msg send|read [--ack]`
- Inbox `inbox/<agent-id>.jsonl` + `.cursor`
- Worker delivery via `send_to_session` + inbox log
- Orchestrator delivery: append; idle+quiet 10 s → nudge; turn-end / prompt-submit digest hooks
- `ask` → sender `blocked`; `answer` clears

### Out of scope

- Native wake (Claude Monitor / opencode plugin)
- `worker spawn` / `orch new` (#246, #247)
- orch.sh hard cut (#236 → slice 3)
- Fleet unread badges (slice 4)

## Technical approach

Reuse `send_to_session` and `cdamux state` for status side-effects. Extend
`--cdamux-hooks` status adapters with turn-end digest injection where each client
supports it (Cursor: `stop.followup_message`).

## Tasks

- [x] **Inbox + CLI**: `msg send` / `msg read [--ack]` round-trip ≤ 2 KB lines with kinds and optional `ref` (acceptance: unit/CLI tests on temp `CDAMUX_HOME`)
- [x] **Worker delivery**: send to worker pastes pane and appends inbox (acceptance: throwaway tmux test)
- [x] **Orchestrator delivery**: append; when target `idle` and pane quiet ≥ 10 s, type nudge line (acceptance: test with injectable quiet window)
- [x] **Hooks**: turn-end continues orch turn with unread digest; prompt-submit adds digest where supported; document Cursor `beforeSubmitPrompt` limit (acceptance: hook fixtures + docs)
- [x] **ask/answer**: `ask` sets sender `blocked` via state; `answer` clears (acceptance: unit test)
- [x] **Verify**: `nix develop -c bash scripts/verify.sh` green

## Risks & mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Cursor cannot inject on `beforeSubmitPrompt` | Use `stop.followup_message` + idle nudge |
| Nudge races with user typing | Require status `idle` + 10 s quiet pane |

## Test plan

Seams: (1) inbox I/O, (2) send→pane+inbox, (3) idle nudge, (4) ask/answer status, (5) hook digest fixtures.

- [x] Temp home CLI tests
- [x] Throwaway tmux delivery / nudge
- [x] Hook payload fixtures
- [x] `nix develop -c bash scripts/verify.sh`

## Verification

- [x] Lint / test / build per project conventions
- [x] Update foam / ADR only if a new decision was made
