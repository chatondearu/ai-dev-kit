#!/usr/bin/env bash
# Tests new-prd.sh / new-plan.sh in a temp repo with a stub gh (no network).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../../../../.." && pwd)"
NEW_PRD="$ROOT/skills/core/cda-prd/scripts/new-prd.sh"
NEW_PLAN="$ROOT/skills/core/cda-plan/scripts/new-plan.sh"
SCAFFOLD="$ROOT/skills/core/cda-foam/templates/scaffold"
FAILS=0

check() { # check DESC CMD...
  local d="$1"; shift
  if "$@" >/dev/null 2>&1; then echo "ok: $d"; else echo "FAIL: $d" >&2; FAILS=$((FAILS + 1)); fi
}

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
R="$TMP/repo"
mkdir -p "$TMP/bin" "$R/prd" "$R/plans" "$R/foam/decisions"
cp "$SCAFFOLD/foam/index.md" "$R/foam/index.md"
printf '# ADR-001\nThe dashboard widget layout is fixed.\n' >"$R/foam/decisions/ADR-001-widget.md"
printf '# ADR-002\nUnrelated topic.\n' >"$R/foam/decisions/ADR-002-other.md"
git -C "$R" init -q

# Stub gh: `issue view` prints the canned JSON.
printf '#!/usr/bin/env bash\ncat "%s/issue.json"\n' "$TMP" >"$TMP/bin/gh"
chmod +x "$TMP/bin/gh"
export PATH="$TMP/bin:$PATH"

TITLE='Fix C:\new "q" dashboard widget'
jq -n --arg t "$TITLE" '{number:7,title:$t,url:"https://github.com/o/r/issues/7",state:"OPEN",
  body:"## Intro\nx\n\n## Goal\nfirst goal line\nlast goal line"}' >"$TMP/issue.json"

cd "$R"
bash "$NEW_PRD" 7 s >/dev/null
PRD="$R/prd/PRD-7-s.md"
check "prd heading keeps backslashes and quotes" grep -qxF "# PRD-7 — $TITLE" "$PRD"
check "prd keeps last line of a final Goal section" grep -qxF "last goal line" "$PRD"
check "index row added" grep -qF "[[../prd/PRD-7-s|PRD-7-s]] | draft |" foam/index.md
check "index placeholder removed" bash -c '! grep -q "none yet" foam/index.md'
check "index row sits above ## Tags" bash -c \
  '[ "$(grep -n "PRD-7-s" foam/index.md | head -1 | cut -d: -f1)" -lt "$(grep -n "^## Tags" foam/index.md | cut -d: -f1)" ]'
bash "$NEW_PRD" --force 7 s >/dev/null
check "--force does not duplicate index row" bash -c '[ "$(grep -c "PRD-7-s" foam/index.md)" = 1 ]'

bash "$NEW_PLAN" 7 s >/dev/null
PLAN="$R/plans/PLAN-7-s.md"
check "plan heading keeps backslashes and quotes" grep -qxF "# PLAN-7 — $TITLE" "$PLAN"
check "plan lists matching ADR" grep -qF "[[../foam/decisions/ADR-001-widget|ADR-001-widget]]" "$PLAN"
check "plan skips unrelated ADR" bash -c '! grep -q "ADR-002" "$1"' _ "$PLAN"

if [ "$FAILS" -gt 0 ]; then echo "FAILS=$FAILS" >&2; exit 1; fi
echo "summary: all ok"
