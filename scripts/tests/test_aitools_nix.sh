#!/usr/bin/env bash
# Manual check (not in verify.sh): fetches the moshi-hook release from the CDN, so it needs network.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"

echo "== bash -n bootstrap =="
bash -n "$ROOT/aitools/scripts/moshi-hook-bootstrap.sh"

if ! command -v nix >/dev/null; then
  echo "SKIP: nix not found, package build not checked"
  exit 0
fi
system="$(nix eval --impure --raw --expr builtins.currentSystem)"
echo "== build moshi-hook package ($system) =="
nix build --no-link "$ROOT#packages.$system.moshi-hook" -L

echo "ok"
