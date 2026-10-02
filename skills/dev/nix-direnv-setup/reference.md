# Nix + direnv — reference templates

Copy and adapt. Replace `<project>` and version pins from the target repo.

## Vue / Nuxt / TypeScript (PATH guard)

Matches `beamy`, `vue-maplibre`, `leonardo-downloader`.

```nix
{
  description = "<project> — Vue.js / TypeScript / Nuxt development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    systems.url = "github:nix-systems/default";
    flake-utils = {
      url = "github:numtide/flake-utils";
      inputs.systems.follows = "systems";
    };
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

## pnpm monorepo + Postgres + Docker

Matches `bajometer`, `cda` (without Postgres).

```nix
devShells.default = pkgs.mkShell {
  name = "<project>";
  packages = with pkgs; [
    nodejs_22
    pnpm
    postgresql_16
    docker-compose
    git
  ];
  shellHook = ''
    echo "<project> dev shell — node $(node --version) | pnpm $(pnpm --version)"
  '';
};
```

`.envrc`:

```bash
use flake
dotenv_if_exists .env
[[ -f .envrc.local ]] && source_env .envrc.local
```

## pnpm monorepo with local PNPM_HOME

Matches `cda`.

```nix
shellHook = ''
  export PNPM_HOME="$PWD/.pnpm"
  export PATH="$PNPM_HOME:$PATH"
'';
```

## Python + uv (multi-system, pinned nixpkgs)

Matches `cool-tts-service`. Use when PyPI wheels need a stable GCC / zlib on Linux.

```nix
{
  description = "<project> — flake dev shell (uv + Python 3.11)";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.11";

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in {
      devShells = forAllSystems (system:
        let pkgs = nixpkgs.legacyPackages.${system};
        in {
          default = pkgs.mkShell {
            packages = with pkgs; [
              python311
              uv
              curl
            ];
            shellHook =
              (pkgs.lib.optionalString pkgs.stdenv.isLinux ''
                export LD_LIBRARY_PATH="${
                  pkgs.lib.makeLibraryPath [
                    pkgs.stdenv.cc.cc.lib
                    pkgs.zlib
                  ]
                }''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
              '')
              + ''
                command -v python3 >/dev/null 2>&1 && export UV_PYTHON="''${UV_PYTHON:-$(command -v python3)}"
                echo "<project> dev shell (Python 3.11 + uv)"
              '';
          };
        });
    };
}
```

`.envrc` comment when users hit CXXABI errors:

```bash
# If nix develop fails with CXXABI_1.3.15, try: direnv reload
# or: env -u LD_LIBRARY_PATH nix develop
use flake
```

## Node + Python + corepack + auto venv

Matches `mirabelle-ha-blueprints`.

`.envrc`:

```bash
if ! has nix_direnv; then
  source_url "https://github.com/nix-community/nix-direnv/raw/master/direnvrc" "sha256-MJx9nGHqA7DwuE2d0OwBu3RPt3XHDHBiO3YVXp3D3m8="
fi

use flake
watch_file requirements-test.txt
```

`shellHook` excerpt (corepack pnpm pin + venv):

```bash
export COREPACK_ENABLE_STRICT=0
corepack enable >/dev/null 2>&1 || true
corepack prepare pnpm@10.24.0 --activate >/dev/null 2>&1 || true

if [ ! -d .venv ]; then
  python -m venv .venv
fi
source .venv/bin/activate
```

Read `package.json` `packageManager` for the exact `corepack prepare pnpm@…` version.

## Minimal ops / scripts only

Matches `my-cooli-claw`.

```nix
{
  description = "<project> — dev tools";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in {
      devShells = forAllSystems (system:
        let pkgs = nixpkgs.legacyPackages.${system};
        in {
          default = pkgs.mkShellNoCC {
            packages = with pkgs; [ git jq shellcheck ];
          };
        });
    };
}
```

## Node version mapping (nixpkgs attr)

| Target | nixpkgs attribute |
|--------|-------------------|
| Node 24 | `nodejs_24` |
| Node 22 | `nodejs_22` |
| Node 20 | `nodejs_20` |

Match `package.json` `engines.node` when present.

## Checklist after bootstrap

- [ ] `nix flake check` passes
- [ ] `nix develop -c <primary-tool> --version` prints expected version
- [ ] `.envrc` loads (`direnv allow` on user machine)
- [ ] `flake.lock` generated and committed with `flake.nix`
- [ ] `.envrc.local` in `.gitignore` if `.envrc` references it
