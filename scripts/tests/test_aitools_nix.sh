#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"

echo "== build moshi-hook package =="
nix build --no-link "$ROOT#packages.x86_64-linux.moshi-hook" -L

echo "== bash -n bootstrap =="
bash -n "$ROOT/aitools/scripts/moshi-hook-bootstrap.sh"

echo "ok"
