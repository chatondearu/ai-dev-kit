#!/usr/bin/env bash
# shellcheck shell=bash
# Publish orch run/nodes/worker Agents via `cdamux state` (ADR-011 §5).
# Fail-open: missing/old cdamux → silent no-op (ADR-001). Dual-write until #252.

orch_cdamux_available() {
  command -v cdamux >/dev/null 2>&1
}

# cdamux state set KIND ID JSON — never fails the caller.
orch_cdamux_state_set() {
  local kind="$1" id="$2" json="$3"
  orch_cdamux_available || return 0
  [ -n "$id" ] || return 0
  cdamux state set "$kind" "$id" "$json" >/dev/null 2>&1 || true
}

orch_cdamux_state_append() {
  local kind="$1" id="$2" json="$3"
  orch_cdamux_available || return 0
  [ -n "$id" ] || return 0
  cdamux state append "$kind" "$id" "$json" >/dev/null 2>&1 || true
}

# Mint a-xxxxx (same shape as cdamux-core). Empty when /dev/urandom is unavailable.
orch_cdamux_mint_agent_id() {
  local s
  s="$(tr -dc '0-9a-z' </dev/urandom 2>/dev/null | head -c 5)" || true
  [ "${#s}" -eq 5 ] || return 1
  printf 'a-%s\n' "$s"
}

# Stable agent id for orch worker $1 (cached under .orch/run/).
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

# Mirror the live graph file into $CDAMUX_HOME/runs/<id>.json (+ parent when set).
orch_cdamux_publish_run() {
  local gf parent repo json
  orch_cdamux_available || return 0
  command -v jq >/dev/null 2>&1 || return 0
  [ -n "${ORCH_RUN_ID:-}" ] || return 0
  if declare -F orch_graph_file >/dev/null; then
    gf="$(orch_graph_file)"
  else
    gf="${ORCH_REPO:-.}/.orch/graph/live/${ORCH_RUN_ID}.json"
  fi
  [ -f "$gf" ] || return 0
  parent="${CDAMUX_AGENT_ID:-}"
  repo="$(cd "${ORCH_REPO:-.}" 2>/dev/null && pwd || printf '%s' "${ORCH_REPO:-.}")"
  json="$(jq -c \
    --arg id "$ORCH_RUN_ID" \
    --arg parent "$parent" \
    --arg repo "$repo" \
    '. + {id:$id, repo:(.repo // $repo)}
     + (if $parent == "" then {parent:null} else {parent:$parent} end)' \
    "$gf" 2>/dev/null)" || return 0
  [ -n "$json" ] || return 0
  orch_cdamux_state_set runs "$ORCH_RUN_ID" "$json"
}

# One Agent per orch worker under $CDAMUX_HOME/agents/ (role=worker).
# Args: worker_id phase [message]
orch_cdamux_publish_worker() {
  local wid="$1" phase="$2" msg="${3:-}"
  local aid host client status parent run json
  orch_cdamux_available || return 0
  command -v jq >/dev/null 2>&1 || return 0
  [ -n "$wid" ] || return 0
  aid="$(orch_cdamux_worker_agent_id "$wid")"
  host=""
  if declare -F orch_tmux_on >/dev/null && orch_tmux_on && declare -F orch_tmux_session >/dev/null; then
    host="$(orch_tmux_session "$wid")"
  fi
  client="unknown"
  if [ -n "${ORCH_AGENT_CLI:-}" ] && declare -F orch_cdamux_agent_name >/dev/null; then
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
    --arg message "$msg" \
    '{id:$id, client:$client, host:(if $host=="" then null else $host end),
      role:"worker", status:$status, name:$name,
      parent:(if $parent=="" then null else $parent end),
      run:(if $run=="" then null else $run end)}
     + (if $message=="" then {} else {message:$message} end)')" || return 0
  orch_cdamux_state_set agents "$aid" "$json"
}
