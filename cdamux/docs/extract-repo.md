# Extract cdamux to its own repository

When v0 dogfood passes (see PRD-39 success metrics):

1. Create `github.com/chatondearu/cdamux` (or org repo).
2. Copy `cdamux/` tree (preserve git history optional: `git subtree split`).
3. Add CI: `cargo test`, `cargo clippy`, release binaries (linux x86_64/aarch64, macOS).
4. In ai-dev-kit:
   - `git submodule add git@github.com:chatondearu/cdamux.git cdamux`
   - Or pin version in `aitools` Nix `fetchFromGitHub`.
5. Update ADR-004 with final repo URL; PRD-39 status → shipped.
6. Add `cda.aitools.cdamux.enable` only after second host dogfood.

Until then, develop in-tree and reference paths as `cdamux/` in docs.

## Validation (#48, 2026-10-26)

| Step | Ready | Notes |
| ---- | ----- | ----- |
| 1. Create GitHub repo | Human | [#56](https://github.com/chatondearu/ai-dev-kit/issues/56) |
| 2. Copy / subtree split | Yes | `cdamux/` is self-contained workspace |
| 3. CI (test, clippy, release) | Planned | Add in extracted repo |
| 4. Submodule or Nix pin in kit | After extract | ADR-003: no default `aitools` enable |
| 5. ADR-004 + PRD shipped | After dogfood sign-off | Maintainer confirms metrics |
| 6. Second-host dogfood before Nix | Blocked | Explicit gate in ADR-003 |

Prerequisite: maintainer completes interactive rows in `dogfood.md` (issue #48).
