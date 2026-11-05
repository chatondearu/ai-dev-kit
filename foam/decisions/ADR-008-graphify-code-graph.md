# ADR-008 — Graphify code graph via cda.aitools

- **Status**: accepted
- **Date**: 2026-11-05
- **Tags**: #architecture #tooling #deployment

## Context

Agents re-read large trees before acting. Graphify (PyPI `graphifyy`, Apache-2.0)
builds a local tree-sitter code graph agents can query. It must not be installed
by `install.sh` ([[ADR-001-host-tools-vs-kit|ADR-001]]); packaging belongs in the
opt-in companion ([[ADR-003-cda-aitools|ADR-003]]).

## Decision

1. **`cda.aitools.graphify.enable`** (default off) exports `CDA_GRAPHIFY=1` and
   registers the `/graphify` skill via idempotent `graphify install`.
2. **`graphify.installMethod = auto | nix | uv | none`**: pinned wheel in
   `aitools/nix/graphify-package.nix`; `auto`/`uv` may run `uv tool install
   graphifyy` at activation when the Nix path is incomplete (missing grammars).
3. **No `graphify.hooks.enable`**: `graphify claude install` writes PreToolUse
   hooks and a section into the **cwd** `CLAUDE.md` / `.claude/settings.json`,
   which is unsafe for kit checkouts and activation oneshots. Re-introduce only
   with a proven user-scope path covered by an isolated test.
4. **`cda.aitools.foamHome`** optionally exports `CDA_FOAM_HOME` for skills that
   need a relocatable foam root (e.g. floorplan).
5. Activation is a user oneshot (systemd / launchd), not `install.sh`. The
   activate script runs in a throwaway cwd and, when
   `$CLAUDE_CONFIG_DIR/CLAUDE.md` is a symlink into a managed kit tree, shields
   the target so `graphify install` cannot dirty the repo (skill still installs).

## Alternatives considered

| Alternative | Why rejected |
| ----------- | ------------ |
| Install graphify from `install.sh` | Violates ADR-001 |
| Nix-only with every tree-sitter grammar | Many grammars absent from nixpkgs; `uv` fallback required |
| Ship `graphify.hooks.enable` now | `graphify claude install` is cwd-scoped; no safe user-scope path yet |

## Consequences

- **Positive**: One switch on a Nix host gives PATH + env + skill registration.
- **Negative**: Nix package relaxes missing grammars; full coverage needs `uv`.
  Claude PreToolUse hooks are manual until a user-scope installer exists.
- **Constraint**: Kit skills that read code should honour `CDA_GRAPHIFY=1` (see #189).
  Tests and workers must use temp `HOME` / `CLAUDE_CONFIG_DIR` / `XDG_CONFIG_HOME`
  / `CURSOR_HOME` (never the live maintainer config).

## References

- PRD: [`prd/PRD-186-floorplan-graphify.md`](../../prd/PRD-186-floorplan-graphify.md)
- Issue: [#188](https://github.com/chatondearu/ai-dev-kit/issues/188)
- [[ADR-001-host-tools-vs-kit|ADR-001]], [[ADR-003-cda-aitools|ADR-003]]
