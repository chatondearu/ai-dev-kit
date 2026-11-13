---
name: nix-develop-shell
description: >-
  Use when running project toolchain commands (pnpm, npm, make, cargo, pytest,
  etc.) in a repo that has flake.nix or .envrc. Default: NEVER wrap with nix
  develop. Prefer host PATH, direnv, or activated Mise. nix develop -c is a
  last-resort fallback only — never wrap git/gh/system tools or every command.
---

# Nix development shell

**Default: do not use `nix develop`.**

`nix develop` evaluates/builds a shell (hundreds of ms warm, seconds–minutes
cold), pulls store paths, and saturates disk when agents wrap every `cargo` /
`node` / `verify` call. Parallel agents make this catastrophic.

## Hard rules

1. **Never** prefix `git`, `gh`, `rg`, `fd`, `alejandra`, `journalctl`, `df`,
   `ps`, editors, or other OS/NixOS tools with `nix develop` — a root
   `flake.nix` alone is **not** a reason to wrap.
2. **Never** wrap “just in case” because `flake.nix` or `.envrc` exists.
3. **Never** open a fresh `nix develop -c` for each small command in a loop
   (test → fix → test). Prefer host/Mise PATH, or **one** outer shell for a
   batch of toolchain steps.
4. **At most one** `nix develop -c …` retry after a real `command not found`
   (or wrong-version) for a **project toolchain** binary — not before the first
   attempt on host/Mise.

## Decision ladder (stop at the first that works)

1. **Host / system tools — run bare.**
2. **Already in the right env — run bare.** If `IN_NIX_SHELL` is set, direnv
   has loaded the project, or **activated Mise** put the right tools on `PATH`
   (version OK), call the tool directly. Prefer activate / one outer
   `mise exec`|`mise run` — **not** Mise shims as the primary agent PATH.
3. **Try host (or activated Mise) first** for `pnpm`, `npm`, `node`, `python`,
   `uv`, `cargo`, `pytest`, etc.
4. **Only if missing or wrong version — then wrap once.**  
   `nix develop -c <that command>` (or one outer `mise exec` / `mise run` when
   `mise.toml` pins the tool). Prefer batching:  
   `nix develop -c bash -c 'cd cdamux && cargo test -p cdamux-cli && cargo test -p cdamux-core'`  
   instead of two separate develops.
5. **No usable `devShell` — do not wrap.** If `nix develop -c true` fails
   (flake has no `devShells.default`), stay on host PATH and say so. Do not
   retry every command through a broken develop.

Presence of `flake.nix` or `.envrc` means “a project shell *may* exist”, not
“wrap every shell invocation”. Optional `mise.toml` pins language tools only
(ADR-013); it does not replace flake packages or modules.

## Command patterns

```bash
# Prefer (host / direnv / activated Mise)
cargo test -p cdamux-cli
bash scripts/verify.sh

# Optional Mise pins (one outer wrap when activate is not loaded)
mise exec -- cargo test -p cdamux-cli
mise run verify

# Last resort only — after command not found / bad version
nix develop -c bash -c 'cd cdamux && cargo test -p cdamux-cli'
nix develop -c bash scripts/verify.sh
```

## direnv vs flake vs Mise

- **`.envrc` (direnv)**: prefer the loaded environment; wrap only after
  `command not found` or a bad version.
- **Activated Mise** (`mise activate` / direnv+mise): same as host PATH once
  tools are real binaries; do not use shim-primary PATH for agents.
- **`flake.nix` with `devShells.default`**: wrap project toolchain only when
  host/Mise failed — **once per batch**, not per keystroke.
- **`flake.nix` without a devShell** (e.g. NixOS config flakes): never use
  `nix develop` for day-to-day commands.
- **Combined**: host → activated Mise / direnv → rare `nix develop` fallback.

## Verification

If a project command fails with `command not found` for a **toolchain** binary,
retry **once** inside `nix develop -c …` (or one outer `mise exec` / `mise run`
when `mise.toml` pins that tool). Do not pre-wrap. Do not nest verify scripts
that each re-enter `nix develop`.
