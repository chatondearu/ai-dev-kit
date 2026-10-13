#!/usr/bin/env bash
# Every tracked kit shell script must parse (bash -n).
set -euo pipefail
TROOT="$(cd "$(dirname "$0")" && pwd)"
source "$TROOT/helpers.sh"
ROOT="$(cd "$TROOT/../../../../.." && pwd)"

while IFS= read -r f; do
  if err="$(bash -n "$ROOT/$f" 2>&1)"; then
    PASSES=$((PASSES + 1))
  else
    FAILS=$((FAILS + 1))
    printf 'FAIL bash -n %s\n%s\n' "$f" "$err" >&2
  fi
done < <(git -C "$ROOT" ls-files 'skills/*.sh')

summary
