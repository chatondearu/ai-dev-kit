#!/usr/bin/env bash
# Regression: `setup-agents.sh --init-foam --force` must never delete project memory.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../../../../.." && pwd)"
SETUP="$ROOT/skills/core/cda-agents/scripts/setup-agents.sh"
INIT="$ROOT/skills/core/cda-foam/scripts/init.sh"
FAILS=0

check() { # check <desc> <test args...>
  local desc="$1"
  shift
  if "$@"; then echo "ok: $desc"; else echo "FAIL: $desc" >&2; FAILS=$((FAILS + 1)); fi
}

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
export HOME="$TMP/home"
mkdir -p "$HOME"

seed() { # seed <repo>
  mkdir -p "$1/prd" "$1/.cursor" "$1/foam/decisions"
  git -C "$1" init -q
  echo mine >"$1/prd/PRD-7-mine.md"
  echo '{}' >"$1/.cursor/mcp.json"
  echo adr >"$1/foam/decisions/ADR-001-x.md"
}
intact() { # intact <repo>
  [ -f "$1/prd/PRD-7-mine.md" ] && [ -f "$1/.cursor/mcp.json" ] && [ -f "$1/foam/decisions/ADR-001-x.md" ] &&
    [ "$(cat "$1/prd/PRD-7-mine.md")" = mine ]
}

seed "$TMP/a"
bash "$SETUP" --init-foam --force "$TMP/a" >/dev/null
check "setup-agents --init-foam --force keeps memory" intact "$TMP/a"

seed "$TMP/b"
bash "$INIT" --force "$TMP/b" >/dev/null
check "init.sh --force keeps memory" intact "$TMP/b"
check "init.sh --force still copies templates" test -f "$TMP/b/prd/template.md"

# A target path with quote/dollar must not be re-parsed by a shell.
seed "$TMP/c\$x\"y"
bash "$INIT" --force "$TMP/c\$x\"y" >/dev/null
check "init.sh --force handles odd path" intact "$TMP/c\$x\"y"

[ "$FAILS" -eq 0 ] || { echo "FAILS=$FAILS" >&2; exit 1; }
echo "test_setup_agents.sh: all passed"
