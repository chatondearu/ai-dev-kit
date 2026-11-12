#!/usr/bin/env bash
# shellcheck shell=bash
# Publish orch run/nodes/worker Agents via `cdamux state` (ADR-011 §5).
# Fail-open: missing/old cdamux → silent no-op (ADR-001). Hard-cut: no .orch graph/status (#252).

orch_cdamux_available() {
  command -v cdamux >/dev/null 2>&1
}

# Resolved state root (env / XDG). Empty when unset — callers must not invent a home.
orch_cdamux_home() {
  if [ -n "${CDAMUX_HOME:-}" ]; then
    printf '%s\n' "$CDAMUX_HOME"
  elif [ -n "${XDG_STATE_HOME:-}" ]; then
    printf '%s/cdamux\n' "$XDG_STATE_HOME"
  elif [ -n "${HOME:-}" ]; then
    printf '%s/.local/state/cdamux\n' "$HOME"
  fi
}

# cdamux state set KIND ID JSON — never fails the caller.
orch_cdamux_state_set() {
  local kind="$1" id="$2" json="$3"
  orch_cdamux_available || return 0
  [ -n "$id" ] || return 0
  cdamux state set "$kind" "$id" "$json" >/dev/null 2>&1 || true
}

# cdamux state get KIND ID → stdout. Returns 1 when missing/unavailable.
orch_cdamux_state_get() {
  local kind="$1" id="$2"
  orch_cdamux_available || return 1
  [ -n "$id" ] || return 1
  cdamux state get "$kind" "$id" 2>/dev/null
}

# Product client name for Agent.client / @cdamux_agent (ADR-011 §1):
# claude|cursor|opencode|codex. Unknown basenames pass through (tests / stubs).
orch_cdamux_agent_name() {
  local base
  base="$(command -p basename "${1}")"
  case "$base" in
    agent|cursor-agent) printf 'cursor\n';;
    claude|claude-code) printf 'claude\n';;
    opencode) printf 'opencode\n';;
    codex) printf 'codex\n';;
    *) printf '%s\n' "$base";;
  esac
}

# Mint a-xxxxx (same shape as cdamux-core). Empty when /dev/urandom is unavailable.
orch_cdamux_mint_agent_id() {
  local s
  s="$(tr -dc '0-9a-z' </dev/urandom 2>/dev/null | head -c 5)" || true
  [ "${#s}" -eq 5 ] || return 1
  printf 'a-%s\n' "$s"
}

# Stable agent id for orch worker $1 (cached under .orch/run/ as a work artefact).
orch_cdamux_worker_agent_id() {
  local wid="$1" dir f id
  dir="${ORCH_REPO:-.}/.orch/run"
  mkdir -p "$dir"
  f="$dir/${wid}.agent_id"
  if [ -f "$f" ]; then
    id="$(tr -d '[:space:]' <"$f")"
    [ -n "$id" ] && { printf '%s\n' "$id"; return 0; }
  fi
  id="$(orch_cdamux_mint_agent_id)" || id="a-w$(printf '%s' "$wid" | tr -c 'A-Za-z0-9' '_')"
  printf '%s\n' "$id" >"$f"
  printf '%s\n' "$id"
}

# Map orch progress phase → ADR-011 agent status.
orch_cdamux_status_from_phase() {
  case "$1" in
    done) printf 'done\n';;
    blocked) printf 'blocked\n';;
    pending) printf 'idle\n';;
    *) printf 'working\n';;
  esac
}

# Path of runs/<id>.json under the resolved home (for readers/tests). Empty without home/id.
orch_cdamux_run_path() {
  local home id
  home="$(orch_cdamux_home)"
  id="${1:-${ORCH_RUN_ID:-}}"
  [ -n "$home" ] && [ -n "$id" ] || return 1
  id="$(printf '%s' "$id" | tr -c 'A-Za-z0-9._-' '_')"
  printf '%s/runs/%s.json\n' "$home" "$id"
}

# Lock file for run RMW (repo artefact — not a second status store).
orch_cdamux_run_lock() {
  local id="${1:-${ORCH_RUN_ID:-}}"
  [ -n "$id" ] || return 1
  id="$(printf '%s' "$id" | tr -c 'A-Za-z0-9._-' '_')"
  mkdir -p "${ORCH_REPO:-.}/.orch/run"
  printf '%s/.orch/run/%s.graph.lock\n' "${ORCH_REPO:-.}" "$id"
}

# Write run JSON via cdamux state (adds id/parent/repo). No-op without cdamux.
orch_cdamux_publish_run_json() {
  local json="$1" parent repo
  orch_cdamux_available || return 0
  command -v jq >/dev/null 2>&1 || return 0
  [ -n "${ORCH_RUN_ID:-}" ] || return 0
  [ -n "$json" ] || return 0
  parent="${CDAMUX_AGENT_ID:-}"
  repo="$(cd "${ORCH_REPO:-.}" 2>/dev/null && pwd || printf '%s' "${ORCH_REPO:-.}")"
  json="$(printf '%s' "$json" | jq -c \
    --arg id "$ORCH_RUN_ID" \
    --arg parent "$parent" \
    --arg repo "$repo" \
    '. + {id:$id, run_id:(.run_id // $id), repo:(.repo // $repo)}
     + (if $parent == "" then {parent:null} else {parent:$parent} end)' \
    2>/dev/null)" || return 0
  [ -n "$json" ] || return 0
  orch_cdamux_state_set runs "$ORCH_RUN_ID" "$json"
}

# One Agent per orch worker under $CDAMUX_HOME/agents/ (role=worker).
# Args: worker_id phase [message] [optional agent jq object / phases from progress]
orch_cdamux_publish_worker() {
  local wid="$1" phase="$2" msg="${3:-}" agent_extra="${4:-}"
  local aid host client status parent run json tmux_session log
  [ -n "$agent_extra" ] || agent_extra='{}'
  orch_cdamux_available || return 0
  command -v jq >/dev/null 2>&1 || return 0
  [ -n "$wid" ] || return 0
  aid="$(orch_cdamux_worker_agent_id "$wid")"
  host=""
  tmux_session=""
  log=""
  if declare -F orch_tmux_on >/dev/null && orch_tmux_on && declare -F orch_tmux_session >/dev/null; then
    host="$(orch_tmux_session "$wid")"
    tmux_session="$host"
    declare -F orch_tmux_log >/dev/null && log="$(orch_tmux_log "$wid")"
  fi
  client="unknown"
  if [ -n "${ORCH_AGENT_CLI:-}" ]; then
    client="$(orch_cdamux_agent_name "$ORCH_AGENT_CLI")"
  fi
  status="$(orch_cdamux_status_from_phase "$phase")"
  parent="${CDAMUX_AGENT_ID:-}"
  run="${ORCH_RUN_ID:-}"
  json="$(jq -cn \
    --arg id "$aid" \
    --arg client "$client" \
    --arg host "$host" \
    --arg status "$status" \
    --arg parent "$parent" \
    --arg run "$run" \
    --arg name "$wid" \
    --arg phase "$phase" \
    --arg message "$msg" \
    --arg tmux_session "$tmux_session" \
    --arg log "$log" \
    --arg updated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ" 2>/dev/null || date -u)" \
    --argjson extra "$agent_extra" \
    '{id:$id, client:$client, host:(if $host=="" then null else $host end),
      role:"worker", status:$status, name:$name, phase:$phase,
      parent:(if $parent=="" then null else $parent end),
      run:(if $run=="" then null else $run end),
      updated_at:$updated_at}
     + (if $message=="" then {} else {message:$message} end)
     + (if $tmux_session=="" then {} else {tmux_session:$tmux_session, log:$log} end)
     + ($extra | with_entries(select(.value != null)))')" || return 0
  orch_cdamux_state_set agents "$aid" "$json"
}
