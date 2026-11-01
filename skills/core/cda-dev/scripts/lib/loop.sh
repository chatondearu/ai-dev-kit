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

orch_loop_worker() {
  local wt="$1" prompt="$2"
  if orch_graph_enabled 2>/dev/null && type orch_graph_worker >/dev/null 2>&1; then
    orch_graph_worker "$wt" "$prompt"
    return $?
  fi
  local max="${ORCH_MAX_ITER:-10}" i=1
  local agent_rc verify_rc vcmd wid="${ORCH_WORKER_ID:-}"
  local base_prompt="$prompt" vlog="$wt/.orch/verify.log"
  while [ "$i" -le "$max" ]; do
    if [ -n "$wid" ] && type orch_progress_set >/dev/null 2>&1; then
      orch_progress_set "$wid" agent "iter ${i}/${max}"
    else
      printf 'orch loop iter %s/%s\n' "$i" "$max" >&2
    fi
    rm -f "$wt/.orch/agent-ok" 2>/dev/null || true
    agent_rc=0
    orch_agent_run "$wt" "$prompt" || agent_rc=$?
    if ! vcmd="$(orch_resolve_verify "$wt")"; then
      if [ -n "$wid" ] && type orch_progress_set >/dev/null 2>&1; then
        orch_progress_set "$wid" blocked "$ORCH_NO_VERIFY_REASON"
      else
        printf 'blocked: %s\n' "$ORCH_NO_VERIFY_REASON" >&2
      fi
      return 3
    fi
    if [ -n "$wid" ] && type orch_progress_set >/dev/null 2>&1; then
      orch_progress_set "$wid" verify "${vcmd%%$'\n'*}"
    fi
    mkdir -p "$wt/.orch"
    orch_run_verify "$wt" 2>&1 | tee "$vlog" >&2
    verify_rc="${PIPESTATUS[0]}"
    if [ "$verify_rc" -eq 0 ]; then
      if [ "$vcmd" = "true" ] && [ "$agent_rc" -ne 0 ] && [ ! -f "$wt/.orch/agent-ok" ]; then
        if [ -n "$wid" ] && type orch_progress_set >/dev/null 2>&1; then
          orch_progress_set "$wid" agent "retry after agent fail (${i}/${max})"
        else
          printf 'orch loop: agent failed and verify is fallback true (iter %s/%s)\n' "$i" "$max" >&2
        fi
      else
        if [ -z "$wid" ] || ! type orch_progress_set >/dev/null 2>&1; then
          printf 'verify ok\n' >&2
        fi
        return 0
      fi
    else
      # Retry with the failing output instead of re-sending the same prompt blind.
      if type orch_verify_retry_prompt >/dev/null 2>&1; then
        prompt="$(orch_verify_retry_prompt "$base_prompt" "$wt" "$vlog")"
      fi
    fi
    i=$((i + 1))
  done
  if [ -z "$wid" ] || ! type orch_progress_set >/dev/null 2>&1; then
    printf 'blocked\n' >&2
  fi
  return 1
}

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
