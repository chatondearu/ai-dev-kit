#!/usr/bin/env bash
# aitools nix hygiene + graphify option schema eval (network only for optional package build).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"

echo "== bash -n bootstrap scripts =="
bash -n "$ROOT/aitools/scripts/moshi-hook-bootstrap.sh"
bash -n "$ROOT/aitools/scripts/graphify-activate.sh"

if ! command -v nix >/dev/null; then
  echo "SKIP: nix not found"
  exit 0
fi

echo "== graphify option schema =="
nix eval --impure --expr "
  let
    flake = builtins.getFlake \"path:$ROOT\";
    pkgs = import flake.inputs.nixpkgs { system = builtins.currentSystem; };
    lib = pkgs.lib;
    schema = import $ROOT/aitools/nix/common.nix { inherit lib; };
  in
    assert schema ? graphify;
    assert schema.graphify ? enable;
    assert schema.graphify ? installMethod;
    assert schema.graphify.hooks ? enable;
    assert schema ? foamHome;
    \"ok\"
"

echo "== graphify package parses =="
nix-instantiate --parse "$ROOT/aitools/nix/graphify-package.nix" >/dev/null

# Optional heavy build (CDN / PyPI); skip unless CDA_TEST_AITOOLS_BUILD=1
if [ "${CDA_TEST_AITOOLS_BUILD:-0}" = "1" ]; then
  system="$(nix eval --impure --raw --expr builtins.currentSystem)"
  echo "== build moshi-hook package ($system) =="
  nix build --no-link "$ROOT#packages.$system.moshi-hook" -L
  echo "== build graphify package ($system) =="
  nix build --no-link "$ROOT#packages.$system.graphify" -L
fi

echo "ok"
