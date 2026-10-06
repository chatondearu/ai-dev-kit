#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")" && pwd)"
fail=0
for t in "$ROOT"/test_*.sh; do
  [ -e "$t" ] || continue
  echo "# $t"
  bash "$t" || fail=1
done
exit "$fail"
