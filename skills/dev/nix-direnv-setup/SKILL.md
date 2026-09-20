---
name: nix-direnv-setup
description: >-
  Prepares flake.nix and .envrc for a project's reproducible Nix dev shell on
  this machine. Use when bootstrapping Nix/direnv on a new repo, adding or
  updating a dev shell, pinning Node/pnpm/Python toolchains, or when the user
  asks to set up nix + direnv like their other projects.
---

# Nix + direnv project setup

Complements [nix-develop-shell](../nix-develop-shell/SKILL.md) (how to **run** commands
inside the shell). This skill covers how to **author** `flake.nix` and `.envrc`.

## Before writing files

1. **Inspect the repo** — do not guess the stack:
   - `package.json` / `pnpm-workspace.yaml` → Node version, `packageManager` (pnpm pin)
   - `pyproject.toml` / `requirements*.txt` / `uv.lock` → Python version, `uv`
   - `Cargo.toml`, `go.mod`, `Dockerfile`, `docker-compose.yml` → extra native tools
   - Existing `flake.nix` / `.envrc` → extend or replace only what the user asked for
2. **Check sibling projects** under `/hdd/dev/` for a matching stack (copy patterns, not blind copy-paste).
3. **Never overwrite** an existing `flake.nix` or `.envrc` without explicit user approval.

## Defaults on this machine

| Choice | Default | When to deviate |
|--------|---------|-----------------|
| nixpkgs input | `github:NixOS/nixpkgs/nixos-unstable` | Pin `nixos-24.11` / `nixos-25.11` when wheels or GCC matter (Python native deps) |
| Flake layout | `flake-utils` + `eachDefaultSystem` | Use `forAllSystems` without flake-utils for multi-platform shells with no extra inputs |
| Node | `nodejs_22` + `pnpm` | Match `engines.node` / `.nvmrc` / `packageManager` in `package.json` |
| direnv | `use flake` | Add `dotenv_if_exists`, `.envrc.local`, or `nix-direnv` when needed (see below) |
| VCS | commit `flake.nix`, `flake.lock`, `.envrc` | gitignore `.envrc.local` only |

## Workflow

```
Task progress:
- [ ] 1. Detect stack and required packages
- [ ] 2. Choose flake layout and nixpkgs channel
- [ ] 3. Write flake.nix (devShells.default)
- [ ] 4. Write .envrc
- [ ] 5. Verify: nix flake check && nix develop -c <smoke command>
- [ ] 6. Remind user: direnv allow (if they use direnv interactively)
```

### Step 1 — Package list

Derive `packages` for `pkgs.mkShell` from the project:

| Stack | Typical packages | shellHook extras |
|-------|------------------|------------------|
| Vue / Nuxt / Node | `nodejs_22`, `pnpm`, `git` | PATH prefers nixpkgs pnpm over corepack shim |
| pnpm monorepo | same + optional `postgresql_*`, `docker-compose` | `PNPM_HOME="$PWD/.pnpm"` on PATH |
| Python (uv) | `python311` or `python312`, `uv`, `curl` | `UV_PYTHON`, Linux `LD_LIBRARY_PATH` for wheels |
| Python + Node UI | Python + `nodejs_22`, `ffmpeg-headless`, `libsndfile` | document `uv venv` + npm steps in shellHook echo |
| Shell / ops | `git`, `jq`, `shellcheck` | minimal echo |
| Docs (pandoc) | `pandoc`, texlive subset, `weasyprint` | one-time template download in shellHook |

Add LSP servers only when the user works in Helix/Neovim and asks for them (`typescript-language-server`, `vue-language-server`, etc.).

### Step 2 — flake.nix skeleton

Use the **Vue/Node default** unless the project needs another template from [reference.md](reference.md):

```nix
{
  description = "<project> — development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { nixpkgs, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
      in {
        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            nodejs_22
            pnpm
            git
          ];

          shellHook = ''
            export PATH="${pkgs.pnpm}/bin:${pkgs.nodejs_22}/bin:$PATH"
            echo "[nix develop] node=$(node --version) pnpm=$(pnpm --version)"
          '';
        };
      });
}
```

Rules:

- Set `description` to a short human-readable project label.
- Prefer `pkgs.mkShell`; use `mkShellNoCC` only when the C compiler must not leak into the environment.
- Keep `shellHook` echoes short and actionable (versions + next commands).
- Do not add formatter/checks outputs unless the user asked for a full flake.

### Step 3 — .envrc

**Minimal (most repos):**

```bash
use flake
```

**With app secrets for CLI tools** (drizzle-kit, prisma, etc.):

```bash
use flake
dotenv_if_exists .env
```

**With local overrides** (gitignored):

```bash
use flake
dotenv_if_exists .env
[[ -f .envrc.local ]] && source_env .envrc.local
```

**Heavy Python / slow reloads** — use `nix-direnv` + `watch_file` (see mirabelle-ha-blueprints pattern in [reference.md](reference.md)).

**Optional guard** when Nix may be absent (CI checkouts, contributors without Nix):

```bash
if command -v nix >/dev/null 2>&1; then
  use flake
fi
```

### Step 4 — Verify

Run from the project root:

```bash
nix flake check
nix develop -c node --version    # or python, uv, pnpm, etc.
nix develop -c pnpm install      # when Node project; only if user asked to validate end-to-end
```

If `nix develop` fails with `CXXABI_1.3.15` or stale `LD_LIBRARY_PATH`, retry with:

```bash
env -u LD_LIBRARY_PATH nix develop -c <command>
```

Document that fix in `.envrc` comments when the project uses Python wheels on NixOS.

### Step 5 — Git

- Add and commit `flake.nix`, `flake.lock`, `.envrc`.
- Ensure `.gitignore` contains `.envrc.local` (not `.envrc`).
- Do **not** commit unless the user asked.

### Step 6 — direnv (interactive)

Tell the user once:

```bash
direnv allow
```

After edits to `flake.nix`, run `direnv reload` or re-enter the directory.

## Anti-patterns

- Do not use `nix-shell` / `shell.nix` for new projects — flakes + `devShells.default` only.
- Do not pin Node via `corepack` alone without PATH guard — nixpkgs `pnpm` must win over a broken corepack shim.
- Do not add `ai-dev-kit` / nix-maid wiring here — that is a separate concern (`flake.nix` in ai-dev-kit itself).
- Do not duplicate the full `nix develop -c` command guide — link to `nix-develop-shell` instead.

## Additional material

Stack-specific templates, nix-direnv block, and Python `LD_LIBRARY_PATH` pattern: [reference.md](reference.md).
