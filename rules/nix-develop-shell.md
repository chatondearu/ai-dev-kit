# Nix development shell (global user rule)

**Default: do not use `nix develop`.** It saturates disk when agents wrap every
command. Follow skill **nix-develop-shell** (ADR-013):

1. Run bare on host PATH / direnv / activated Mise first.
2. Never wrap `git`/`gh`/OS tools just because `flake.nix` exists.
3. Only after `command not found` (or wrong version) for a **toolchain** binary
   (`cargo`, `node`, `pnpm`, …): retry **once** with `nix develop -c …`, or one
   outer `mise exec` / `mise run` when `mise.toml` pins that tool.
4. Batch toolchain steps in **one** develop/mise wrap — never a fresh develop
   per small command.

```bash
# Prefer
bash scripts/verify.sh
cargo test -p cdamux-cli

# Last resort
nix develop -c bash scripts/verify.sh
```
