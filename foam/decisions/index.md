# Architecture Decision Records

Status values: `proposed` | `accepted` | `deprecated` | `superseded`.

## Index

| ID | Title | Status | Tags |
| -- | ----- | ------ | ---- |
| [[ADR-001-host-tools-vs-kit|ADR-001]] | Host tools stay outside the kit | accepted | #architecture #tooling |
| [[ADR-002-orch-auto-and-fast-lane|ADR-002]] | Orch auto poller and fast-lane pickup | accepted | #architecture #tooling #autonomy |
| [[ADR-003-cda-aitools|ADR-003]] | cda.aitools companion (experimental) | accepted | #architecture #tooling #deployment |
| [[ADR-004-cdamux|ADR-004]] | cdamux in-tree orchestrator | accepted | #architecture #tooling |
| [[ADR-005-cda-graph-orchestration|ADR-005]] | CDA graph orchestration | accepted | #architecture #tooling #autonomy |
| [[ADR-006-review-gate-fail-closed-verify|ADR-006]] | Independent review gate and fail-closed verify | accepted | #architecture #tooling #quality #autonomy |
| [[ADR-007-cdamux-session-hierarchy|ADR-007]] | cdamux session hierarchy: tmux links, tmux workers, sub-agent ledger | accepted | #architecture #tooling #autonomy |
| [[ADR-008-graphify-code-graph|ADR-008]] | Graphify code graph via cda.aitools | accepted | #architecture #tooling #deployment |
| [[ADR-009-cdamux-session-restore-registry|ADR-009]] | cdamux session restore registry | accepted | #architecture #tooling #resilience |
| [[ADR-010-worker-cli-inheritance-and-tiers|ADR-010]] | Worker CLI inheritance and tiers | accepted | #architecture #tooling #autonomy #cost |
| [[ADR-011-cdamux-unified-state-model|ADR-011]] | cdamux unified state model | accepted | #architecture #tooling #autonomy #resilience |
| [[ADR-012-foam-browser-replaces-notes-sidecar|ADR-012]] | Foam browser replaces Fleet Notes sidecar | accepted | #architecture #cdamux #foam |
| [[ADR-013-mise-alongside-nix-devshell|ADR-013]] | Mise alongside Nix for daily Node/Rust toolchain | accepted | #architecture #tooling |

## Template

Copy [[ADR-template|ADR-template]] → `ADR-NNN-short-slug.md`, number sequentially.
