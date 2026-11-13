---
name: nix-develop-shell
description: >-
  Use when running project toolchain commands (pnpm, npm, make, cargo, pytest,
  etc.) in a repo that has flake.nix or .envrc. Prefer host PATH and an already
  loaded direnv shell; use nix develop -c only as a fallback for missing or
  wrong-version tools — never wrap git/gh/system diagnostics.
---

# Nix development shell

`nix develop` evaluates/builds a shell (often hundreds of ms warm, seconds cold)
and hammers disk when many agents wrap every command. Use it sparingly.

## Decision ladder (stop at the first that works)

1. **Host / system tools — run bare.** Never wrap: `git`, `gh`, `rg`, `fd`,
   `alejandra`, `journalctl`, `df`, `ps`, editors, and other OS/NixOS packages
   already on `PATH`. A root `flake.nix` alone is **not** a reason to wrap.
2. **Already in the right env — run bare.** If `IN_NIX_SHELL` is set, direnv
   has loaded the project, or **activated Mise** put the right tools on `PATH`
   (version OK), call the tool directly. Prefer activate / one outer
   `mise exec`|`mise run` — **not** Mise shims as the primary agent PATH.
3. **Project toolchain missing or wrong version — then wrap.** For `pnpm`,
   `npm`, `node`, `python`, `uv`, `cargo`, `pytest`, etc.: try host (or
   activated Mise) first; on `command not found` or a version that breaks the
   repo, use `nix develop -c`.
4. **No usable `devShell` — do not wrap.** If `nix develop -c true` fails
   (flake has no `devShells.default`), stay on host PATH and say so. Do not
   retry every command through a broken develop.

Presence of `flake.nix` or `.envrc` means “a project shell *may* exist”, not
“wrap every shell invocation”. Optional `mise.toml` pins language tools only
(ADR-013); it does not replace flake packages or modules.

## Command patterns (fallback only)

```bash
# Prefer
pnpm test

# Optional Mise pins (one outer wrap when activate is not loaded)
mise exec -- pnpm test
mise run verify

# Fallback when host / Mise tool missing / wrong
nix develop -c pnpm test
nix develop -c bash -c 'export CI=true && pnpm check'
```

## direnv vs flake vs Mise

- **`.envrc` (direnv)**: prefer the loaded environment; wrap only after
  `command not found` or a bad version.
- **Activated Mise** (`mise activate` / direnv+mise): same as host PATH once
  tools are real binaries; do not use shim-primary PATH for agents.
- **`flake.nix` with `devShells.default`**: wrap project toolchain only when
  host/Mise failed.
- **`flake.nix` without a devShell** (e.g. NixOS config flakes): never use
  `nix develop` for day-to-day commands.
- **Combined**: host → activated Mise / direnv → `nix develop` fallback.

## Verification

If a project command fails with `command not found` for a **toolchain** binary,
retry once inside `nix develop -c …` (or one outer `mise exec` / `mise run`
when `mise.toml` pins that tool). Do not pre-wrap “just in case”.
