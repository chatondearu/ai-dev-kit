#!/usr/bin/env bash
# ADR-019: the integrator runs in a throwaway worktree, never the maintainer's checkout.
set -euo pipefail
TROOT="$(cd "$(dirname "$0")" && pwd)"
source "$TROOT/helpers.sh"
source "$TROOT/../lib/agents.sh"
source "$TROOT/../lib/graph.sh"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
export ORCH_REPO="$TMP/repo" ORCH_TMUX=0 ORCH_RUN_ID=it1 CDAMUX_HOME="$TMP/cdamux"
git -C "$TMP" init -q -b main repo
git -C "$ORCH_REPO" -c user.email=t@t -c user.name=t commit -q --allow-empty -m init
mkdir -p "$ORCH_REPO/.orch"

# Stub agent: records its cwd and writes the report where the prompt says (its checkout).
cat >"$TMP/stub" <<STUB
#!$(command -v bash)
pwd -P >"$TMP/cwd"
mkdir -p .orch && echo synthesis >.orch/integrator-report.md && touch .orch/integrator-ok
STUB
chmod +x "$TMP/stub"
echo prompt >"$TMP/prompt.md"

ORCH_AGENT="$TMP/stub" orch_integrator_agent_run "$TMP/prompt.md" 2>/dev/null
wt="$(orch_integrator_worktree_path)"
assert_eq "$(cat "$TMP/cwd")" "$(cd "$(dirname "$wt")" && pwd -P)/$(basename "$wt")" "integrator ran in its own worktree"
assert_eq "$(cat "$ORCH_REPO/.orch/integrator-report.md")" "synthesis" "report copied back to the repo .orch"
assert_file "$ORCH_REPO/.orch/integrator-ok" "ok marker copied back"
assert_eq "$(test -e "$wt" && echo yes || echo no)" "no" "throwaway worktree removed"
assert_eq "$(git -C "$ORCH_REPO" worktree list | wc -l | tr -d ' ')" "1" "no worktree left registered"
assert_eq "$(git -C "$ORCH_REPO" status --porcelain --untracked-files=no)" "" "maintainer checkout untouched"

summary
