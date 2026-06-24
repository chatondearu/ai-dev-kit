# Nix development shell (global user rule)

Copy the block below into **Cursor Settings → Rules → User Rules** if it is not already applied via the `nix-develop-shell` skill.

---

Before running project commands (install, build, test, lint, format, scripts, etc.) in a workspace:

1. Check if the project root has `flake.nix` or `.envrc` / `.envrc.local` (direnv).
2. If present, run commands through `nix develop -c <command>` (or confirm direnv already loaded the environment).
3. Do not assume `pnpm`, `npm`, `node`, `python`, etc. exist on the host PATH outside the dev shell.

Examples:

- `nix develop -c pnpm install`
- `nix develop -c pnpm test`
- `nix develop -c bash -c 'export CI=true && pnpm check'`

For compound commands, wrap the full script: `nix develop -c bash -c '...'`.

If a command fails with `command not found`, retry inside `nix develop` before other fixes.
