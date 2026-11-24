---
title: "feat(cdamux): events feed changes for neomind P2 (orch chats debuggability)"
status: in-review
priority: P1
updated: 2026-11-24
---

## Goal

The cdamux side of neomind's P2 epic (orchestrator chats correctness and debuggability,
owner decision P2-Q8): every feed item a chat bridge needs is in the journal, a refused
cursor is unambiguous, and a lost prompt or reply leaves a trace.

Epic: [neomind `audit-p2-orch-chats-debuggability`](https://github.com/chatondearu/neomind/blob/main/foam/board/audit-p2-orch-chats-debuggability.md)
· cross-repo card [`audit-orch-kit-cross-repo-asks`](https://github.com/chatondearu/neomind/blob/main/foam/board/audit-orch-kit-cross-repo-asks.md)
(audit findings M5, L4, L5, L6, M1).

## Scope

1. `delivered` events item (`id`, `agent`) when a `say` prompt is pasted (M5).
2. `events --follow` reads only appended bytes; full read on rotation / reset (L5).
3. Production hook log `$CDAMUX_HOME/logs/hooks.log`: 0600, bounded, ids only (L6).
4. `msg answer --json` gains `relayed` (L4).
5. A tool-only turn is a `status` item (`tool_only: true`), never a `reply` (M1).
6. Refused cursor (expired / ahead): exit 3, stderr prefix `events: cursor refused: `.
7. `say` / `msg answer` read the text from stdin with `-` or no text argument.

Schema stays `v: 1` (additive). Docs: [[../../cdamux/docs/events|events.md]], [[../../cdamux/docs/say|say.md]].

## Acceptance

- [x] Tests first for each item (core unit + CLI + hooks test).
- [x] `cdamux/docs/events.md` and `say.md` document every change.
- [ ] Verify green, PR reviewed and merged.
- [ ] neomind adapts (bridge parses `delivered`, `tool_only`, exit 3, stdin) under P2 brief 1.
