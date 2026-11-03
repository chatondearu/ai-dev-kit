#!/usr/bin/env bash
# shellcheck shell=bash
# Source-only library (no main). Verification loop runner for workers.

orch_render_prompt() {
  local template="$1"
  local dest="$2"
  shift 2
  local content
  content="$(cat "$template")"
  while [ $# -gt 0 ]; do
    local kv="$1"
    local k="${kv%%=*}"
    local v="${kv#*=}"
    content="${content//\{\{$k\}\}/$v}"
    shift
  done
  # Replace any remaining {{...}} placeholders with N/A
  content="$(printf '%s\n' "$content" | sed -E 's/\{\{[A-Za-z0-9_]+\}\}/N\/A/g')"
  mkdir -p "$(dirname "$dest")"
  printf '%s\n' "$content" >"$dest"
}

# Resolve worker prompt context: sets ORCH_PRD_PATH, ORCH_EPIC, ORCH_CONSTRAINTS
# (literal `none` when missing). PRD = prd/PRD-<issue>-*.md, else the PRD of the
# parent named under `## Parent` / `Part of #N` (same lookup as cda-plan/new-plan.sh).
# Optional 3rd arg is the issue body; fetched with gh when omitted.
orch_resolve_context() {
  local root="$1" issue="$2" body="" parent="" n adr
  if [ $# -ge 3 ]; then
    body="$3"
  elif command -v gh >/dev/null 2>&1; then
    body="$(gh issue view "$issue" --json body -q .body 2>/dev/null || true)"
  fi
  ORCH_PRD_PATH="" ORCH_EPIC="none" ORCH_CONSTRAINTS=""
  parent="$(printf '%s' "$body" | tr -d '\r' | awk '
    /^##[[:space:]]+Parent[[:space:]]*$/ { p=1; next }
    p && /^##/ { p=0 }
    (p || /^Part of /) && match($0, /#[0-9]+/) { print substr($0, RSTART+1, RLENGTH-1); exit }')"
  [ -n "$parent" ] && ORCH_EPIC="$parent"
  for n in "$issue" "$parent"; do
    [ -n "$n" ] || continue
    ORCH_PRD_PATH="$(find "$root/prd" -maxdepth 1 -name "PRD-${n}-*.md" 2>/dev/null | head -1)"
    [ -n "$ORCH_PRD_PATH" ] && break
  done
  ORCH_PRD_PATH="${ORCH_PRD_PATH#"$root"/}"
  [ -n "$ORCH_PRD_PATH" ] || ORCH_PRD_PATH="none"
  for adr in "$root"/foam/decisions/ADR-*.md; do
    [ -f "$adr" ] || continue
    case "$adr" in */ADR-template.md) continue ;; esac
    adr="${adr##*/}"
    ORCH_CONSTRAINTS="${ORCH_CONSTRAINTS:+$ORCH_CONSTRAINTS, }${adr%.md}"
  done
  [ -n "$ORCH_CONSTRAINTS" ] || ORCH_CONSTRAINTS="none"
}

# Workers run under `bash -c`: export every harness function (orch_* / _orch_*) so
# no helper goes missing in the child (e.g. _orch_verify_is_allowed for pr-fix).
orch_export_functions() {
  local fn
  for fn in $(compgen -A function); do
    case "$fn" in orch_*|_orch_*) export -f "$fn" ;; esac
  done
}

# One worker loop for both modes (lib/graph.sh): --no-graph only stops current.json
# writes, it never skips the grounding verifier (ADR-006).
orch_loop_worker() { orch_graph_worker "$@"; }

# Independent review gate (ADR-006): run an already rendered reviewer.md prompt in a
# fresh agent call, log to RLOG. Returns 0 only on `REVIEW: approved`;
# `REVIEW: changes_requested` or no verdict → 1 (fail closed).
orch_review_run() {
  local wt="$1" rprompt="$2" rlog="$3"
  orch_agent_run "$wt" "$rprompt" 2>&1 | tee "$rlog" >&2 || true
  case "$(grep -oE 'REVIEW: *(approved|changes_requested)' "$rlog" | tail -n 1)" in
    *approved) return 0 ;;
    *changes_requested) return 1 ;;
    *) printf 'orch review: no REVIEW verdict in output; treating as changes_requested\n' >&2; return 1 ;;
  esac
}

# Append the reviewer findings to a worker prompt; prints the new prompt path.
orch_review_fix_prompt() {
  local prompt="$1" wt="$2" rlog="$3"
  local dest="$wt/.orch/worker-prompt-review-fix.md"
  {
    cat "$prompt"
    printf '\n\n## Review findings (changes_requested)\n\nFix these, re-run verify, commit and push to the same branch. Do not merge.\n\n'
    cat "$rlog"
  } >"$dest"
  printf '%s\n' "$dest"
}

# Bounded background pool shared by orch.sh and graph.sh. Callers run
# `orch_pool_slot MAX`, start a job with `&`, then `orch_pool_add $!`; finish
# with `orch_pool_drain`. Needs bash >= 4.3 for `wait -n`.
ORCH_POOL_PIDS=()

orch_pool_reap() {
  local pid still=()
  for pid in "${ORCH_POOL_PIDS[@]}"; do
    if kill -0 "$pid" 2>/dev/null; then still+=("$pid"); else wait "$pid" 2>/dev/null || true; fi
  done
  ORCH_POOL_PIDS=("${still[@]}")
}

orch_pool_slot() {
  orch_pool_reap
  while [ "${#ORCH_POOL_PIDS[@]}" -ge "$1" ]; do
    # wait -n returns the finished job's status; a failed worker is not an error here.
    wait -n 2>/dev/null || sleep 0.05
    orch_pool_reap
  done
}

orch_pool_add() { ORCH_POOL_PIDS+=("$1"); }

orch_pool_drain() {
  local pid
  for pid in "${ORCH_POOL_PIDS[@]}"; do wait "$pid" || true; done
  ORCH_POOL_PIDS=()
}
