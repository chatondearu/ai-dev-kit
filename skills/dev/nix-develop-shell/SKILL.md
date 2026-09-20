---
name: nix-develop-shell
description: >-
  Use when running shell commands (pnpm, npm, make, cargo, pytest, etc.) in any
  repository. Required when the project root has flake.nix or .envrc/.envrc.local
  (direnv). Wrap commands with nix develop -c instead of calling tools directly.
---

# Nix development shell

Before running project commands (install, build, test, lint, format, scripts, etc.):

1. Check whether the workspace root has `flake.nix` or `.envrc` / `.envrc.local`.
2. If either exists, run commands through `nix develop -c <command>` unless the shell is already inside that environment and tools are verified on `PATH`.
3. Do not assume `pnpm`, `npm`, `node`, `python`, etc. exist on the host `PATH` outside the dev shell.

## Command patterns

```bash
# Single command
nix develop -c pnpm install

# Multiple commands / env vars
nix develop -c bash -c 'export CI=true && pnpm check'

# Subshell for pipelines
nix develop -c bash -c 'pnpm test | tee test.log'
```

## direnv vs flake

- **`flake.nix` only**: always use `nix develop -c …`.
- **`.envrc` (direnv)**: the shell may already be correct; if you get `command not found`, use `nix develop -c …` or confirm direnv loaded.
- **Both**: prefer `nix develop` for agent-run commands so behavior is explicit and reproducible.

## Verification

If a command fails with `command not found` for `pnpm`, `node`, `python`, etc., retry inside `nix develop` before trying other fixes.
