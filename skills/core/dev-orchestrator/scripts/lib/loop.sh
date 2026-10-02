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
  local max="${ORCH_MAX_ITER:-10}" i=1
  local agent_rc vcmd
  while [ "$i" -le "$max" ]; do
    printf 'orch loop iter %s/%s\n' "$i" "$max" >&2
    rm -f "$wt/.orch/agent-ok" 2>/dev/null || true
    agent_rc=0
    orch_agent_run "$wt" "$prompt" || agent_rc=$?
    vcmd="$(orch_resolve_verify "$wt")"
    if orch_run_verify "$wt"; then
      if [ "$vcmd" = "true" ] && [ "$agent_rc" -ne 0 ] && [ ! -f "$wt/.orch/agent-ok" ]; then
        printf 'orch loop: agent failed and verify is fallback true (iter %s/%s)\n' "$i" "$max" >&2
      else
        printf 'verify ok\n' >&2
        return 0
      fi
    fi
    i=$((i + 1))
  done
  printf 'blocked\n' >&2
  return 1
}
