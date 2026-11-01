#!/usr/bin/env bash
# orch_resolve_context: real PRD/EPIC/ADR values in the rendered worker prompt.
set -euo pipefail
TROOT="$(cd "$(dirname "$0")" && pwd)"
source "$TROOT/helpers.sh"
source "$TROOT/../lib/loop.sh"
TPL="$TROOT/../../prompts/worker.md"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
REPO="$TMP/repo"
mkdir -p "$REPO/prd" "$REPO/foam/decisions"
: >"$REPO/prd/PRD-90-epic.md"
: >"$REPO/foam/decisions/ADR-001-first.md"
: >"$REPO/foam/decisions/ADR-002-second.md"
: >"$REPO/foam/decisions/ADR-template.md"

render() { # issue body
  orch_resolve_context "$REPO" "$1" "$2"
  orch_render_prompt "$TPL" "$TMP/out.md" "ISSUE=$1" "WORKTREE=$TMP" "REPO_ROOT=$REPO" \
    "TITLE=t" "PLAN_PATH=none" "BRIEF_PATH=none" "LANE=full" \
    "PRD_PATH=$ORCH_PRD_PATH" "EPIC=$ORCH_EPIC" "CONSTRAINTS=$ORCH_CONSTRAINTS"
  cat "$TMP/out.md"
}

# Issue that owns a PRD
out="$(render 90 "")"
assert_contains "$out" 'prd/PRD-90-epic.md' "own PRD"
assert_contains "$out" 'ADR-001-first, ADR-002-second' "ADR list"
[[ "$out" != *ADR-template* ]] && PASSES=$((PASSES + 1)) || { FAILS=$((FAILS + 1)); echo "FAIL template listed" >&2; }
[[ "$out" != *N/A* ]] && PASSES=$((PASSES + 1)) || { FAILS=$((FAILS + 1)); echo "FAIL N/A left in prompt" >&2; }

# Child issue: PRD and epic come from `## Parent`
out="$(render 97 $'## Parent\n\n#90\n\n## Plan')"
assert_contains "$out" 'prd/PRD-90-epic.md' "parent PRD"
assert_contains "$out" 'parent `#90`' "epic number"

# No PRD anywhere → `none`, not N/A
out="$(render 55 "")"
assert_contains "$out" 'Epic PRD: `none` (parent `#none`)' "no PRD"
rm -f "$REPO"/foam/decisions/*.md
orch_resolve_context "$REPO" 55 ""
assert_eq "$ORCH_CONSTRAINTS" "none" "no ADRs"

summary
