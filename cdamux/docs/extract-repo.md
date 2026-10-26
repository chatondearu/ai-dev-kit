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
