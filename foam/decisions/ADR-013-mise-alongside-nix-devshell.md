# ADR-013 — Mise alongside Nix for daily Node/Rust toolchain

- **Status**: accepted
- **Date**: 2026-11-13
- **Tags**: #architecture #tooling
- **GitHub**: [#287](https://github.com/chatondearu/ai-dev-kit/issues/287)
  (map [#284](https://github.com/chatondearu/ai-dev-kit/issues/284))

## Context

Agents wrapping every command in `nix develop -c` paid heavy disk/I/O cost.
The skill `nix-develop-shell` and `AGENTS.md` already prefer **host PATH**, with
`nix develop` only as fallback. The maintainer asked whether root `flake.nix`
(and related Nix config) could be replaced by **Mise** for a lighter daily loop.

Explore research:

- [#285](https://github.com/chatondearu/ai-dev-kit/issues/285) —
  `research/flake-vs-mise-inventory.md`: only `devShells.default` is daily PATH
  (a); packages (b) and maid/aitools modules (c) are not Mise’s job. Mise is a
  strong substitute for **Node** and **Rust** only.
- [#286](https://github.com/chatondearu/ai-dev-kit/issues/286) —
  `research/mise-agent-loop-costs.md`: host + rare Nix already addresses wrap
  I/O; Mise helps pin Node/Rust without flake eval; **shims** add per-call
  overhead and false `command -v` — prefer activate / one outer `mise exec`.

## Decision

1. **Stance B — Mise alongside Nix**, not a full replacement:
   - Optional **Mise** may pin **Node** and **Rust** for daily agent/operator
     loops (`mise.toml` when introduced).
   - **Keep** the root flake: `devShells.default` as hermetic/CI/macOS fallback
     (`util-linux` / Darwin gaps), packages (`cdamux`, graphify, …), nix-maid
     and `cda.aitools` modules.
2. **How to use Mise** when present:
   - Prefer **`mise activate`** (or direnv+mise) so tools are real binaries on
     PATH.
   - Prefer **one** `mise exec` / `mise run` around verify when pins are
     required — not per-subcommand wraps.
   - **Do not** rely on Mise **shims** as the primary agent PATH.
3. **Agent ladder unchanged in priority**: host / activated Mise PATH → then
   `nix develop -c` fallback (**nix-develop-shell**). A `flake.nix` alone never
   means wrap every command.
4. **Out of stance C**: do not delete `devShells.default` in this decision.
   Slimming the shell (dropping node/cargo from the flake list once Mise pins
   them) is a later implementation choice, not required by this ADR.

## Alternatives considered

| Alternative | Why rejected |
| ----------- | ------------ |
| **A — Nix + host PATH only** | Already mitigates agent I/O; rejected as the *sole* long-term story because the maintainer wants optional language pins without paying full flake shell entry for Node/Rust. |
| **C — Drop `devShell`, Mise = daily shell** | Loses one-lockfile fallback for OS CLIs / Darwin `util-linux` gaps; does not replace (b)/(c). |
| Mise shims on PATH for agents | Documented overhead and `command -v` false positives; worse for chatty scripts. |

## Consequences

- **Positive**: Clear split — Nix owns packages/modules/hermetic shell; Mise may
  own language toolchain pins; agents stay on host/activated PATH first.
- **Negative**: Two optional tool managers (Nix + Mise) to document; version
  skew possible if Mise and nixpkgs Rust/Node diverge.
- **Constraint**: Do not rewrite `aitools/` modules or `install.sh` skill
  delivery in Mise. Implementation of `mise.toml` / `mise run verify` is
  follow-up work under map #284 fog, not implied shipped by this ADR.

## References

- Explore map [#284](https://github.com/chatondearu/ai-dev-kit/issues/284)
- Grill ticket [#287](https://github.com/chatondearu/ai-dev-kit/issues/287)
- Research branches: `research/flake-vs-mise-inventory`,
  `research/mise-agent-loop-costs`
- Skill: `skills/dev/nix-develop-shell/SKILL.md`
- Related: [[ADR-001-host-tools-vs-kit|ADR-001]], [[ADR-003-cda-aitools|ADR-003]]
