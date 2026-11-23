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

# Fail loud if export -f did not expose the worker entry to a child bash.
orch_assert_exported() {
  if ! bash -c 'type orch_loop_worker' >/dev/null 2>&1; then
    printf 'orch: export failed — orch_loop_worker not visible to child bash (declare -F / export -f). Use skill-only fallback or cda-graph graph-node.sh + Cursor Task.\n' >&2
    return 1
  fi
}

# Workers run under `bash -c`: export every harness function (orch_* / _orch_*) so
# no helper goes missing in the child (e.g. _orch_verify_is_allowed for pr-fix).
# Always use declare -F — some agent wrappers ship bash without compgen.
orch_export_functions() {
  local line name
  while IFS= read -r line; do
    name="${line##* }"
    case "$name" in
      orch_*|_orch_*) export -f "$name" ;;
    esac
  done < <(declare -F)
  orch_assert_exported
}

# One worker loop for both modes (lib/graph.sh): --no-graph only stops current.json
# writes, it never skips the grounding verifier (ADR-006).
orch_loop_worker() { orch_graph_worker "$@"; }

# Independent review gate (ADR-006): run an already rendered reviewer.md prompt in a
# fresh agent call at the reviewer tier (default large, never the worker's escalated
# tier), log to RLOG. The verdict is read only from the sentinel file
# .orch/review-verdict (`approved` | `changes_requested`), never from stdout, where the
# reviewed diff itself can print a verdict line (M14). No file → 1 (fail closed).
orch_review_run() {
  local wt="$1" rprompt="$2" rlog="$3" verdict
  rm -f "$wt/.orch/review-verdict" 2>/dev/null || true
  ORCH_TIER= ORCH_ROLE=reviewer orch_agent_run "$wt" "$rprompt" 2>&1 | tee "$rlog" >&2 || true
  verdict="$(tr -d '[:space:]' <"$wt/.orch/review-verdict" 2>/dev/null || true)"
  case "$verdict" in
    approved) return 0 ;;
    changes_requested) return 1 ;;
    *) printf 'orch review: no verdict in .orch/review-verdict; treating as changes_requested\n' >&2; return 1 ;;
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
