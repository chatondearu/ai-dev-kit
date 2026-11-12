#!/usr/bin/env bash
# shellcheck shell=bash
# Source-only: worker status files + optional TTY progress dashboard.
# Dual-writes one Agent per worker via `cdamux state` until #252 (ADR-011 §5).

_ORCH_CDAMUX_STATE_LIB="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/cdamux_state.sh"
# shellcheck source=cdamux_state.sh
source "$_ORCH_CDAMUX_STATE_LIB"
unset _ORCH_CDAMUX_STATE_LIB

# Status line format: PHASE|message  (PHASE = pending|worktree|agent|verify|review|ci|done|blocked)

orch_progress_dir() {
  printf '%s/.orch/run\n' "${ORCH_REPO:-.}"
}

orch_progress_enabled() {
  case "${ORCH_PROGRESS:-auto}" in
    0|off|false|no) return 1;;
    1|on|true|yes) return 0;;
  esac
  # auto: only when stderr is a TTY
  [ -t 2 ]
}

# Machine-readable status for waiting agents (poll `.orch/run/status.json`).
orch_progress_write_json() {
  local id="$1" phase="$2" msg="$3"
  local dir ts json_file agg tmp prev agent
  dir="$(orch_progress_dir)"
  mkdir -p "$dir"
  ts="$(date -u +"%Y-%m-%dT%H:%M:%SZ" 2>/dev/null || date -u)"
  json_file="$dir/${id}.json"
  # ORCH_TMUX=1: point cdamux at the worker's session (pane preview) and log (tail preview).
  local tmux_session="" log=""
  if declare -F orch_tmux_on >/dev/null && orch_tmux_on; then
    tmux_session="$(orch_tmux_session "$id")"
    log="$(orch_tmux_log "$id")"
  fi
  # Parallel workers write here: build in a same-dir temp file, then rename (atomic),
  # so a concurrent `jq -s` never reads a half-written file. $BASHPID: subshell-unique.
  tmp="$json_file.tmp.$BASHPID"
  if command -v jq >/dev/null 2>&1; then
    # `phases`: one {phase, started_at (epoch s)} entry per phase change, so durations
    # are real. Older files lack it; `pending` starts a new run and resets it.
    prev="$(jq -c '.phases // []' "$json_file" 2>/dev/null || true)"
    [ -n "$prev" ] || prev='[]'
    # cli/model/effort/tier: kept from the previous file (until `pending` starts a new
    # run), replaced by $4 (orch_progress_agent).
    agent='{}'
    [ "$phase" = pending ] || agent="$(jq -c '{cli, model, effort, tier} | with_entries(select(.value != null))' "$json_file" 2>/dev/null || true)"
    agent="$(jq -cn --argjson a "${agent:-{\}}" --argjson b "${4:-{\}}" '$a + $b | with_entries(select(.value != null))')"
    jq -n \
      --arg id "$id" \
      --arg phase "$phase" \
      --arg message "$msg" \
      --arg updated_at "$ts" \
      --argjson prev "$prev" \
      --argjson now "$(date +%s)" \
      --arg tmux_session "$tmux_session" \
      --arg log "$log" \
      --argjson agent "$agent" \
      '{id:$id, phase:$phase, message:$message, updated_at:$updated_at,
        phases: (if $phase == "pending" then [{phase:$phase, started_at:$now}]
                 elif ($prev | length) > 0 and $prev[-1].phase == $phase then $prev
                 else ($prev + [{phase:$phase, started_at:$now}])[-40:] end)}
       + (if $tmux_session == "" then {} else {tmux_session:$tmux_session, log:$log} end) + $agent' \
      >"$tmp" && mv "$tmp" "$json_file"
    # Aggregate all workers into one file agents can poll.
    agg="$dir/status.json"
    tmp="$agg.tmp.$BASHPID"
    local files=() f
    for f in "$dir"/*.json; do
      [ -e "$f" ] || continue
      case "$f" in */status.json) continue ;; esac
      files+=("$f")
    done
    if [ "${#files[@]}" -eq 0 ]; then
      jq -n --arg ts "$ts" --arg run "${ORCH_RUN_ID:-}" \
        '{updated_at:$ts, workers:[]} + (if $run == "" then {} else {run_id:$run} end)' >"$tmp" && mv "$tmp" "$agg"
    else
      # run_id: same id as the graph file and the tmux @cdamux_run option.
      jq -s --arg ts "$ts" --arg run "${ORCH_RUN_ID:-}" \
        '{updated_at:$ts, workers:.} + (if $run == "" then {} else {run_id:$run} end)' "${files[@]}" >"$tmp" && mv "$tmp" "$agg"
    fi
  else
    printf '{"id":"%s","phase":"%s","message":"%s","updated_at":"%s"}\n' \
      "$id" "$phase" "$msg" "$ts" >"$tmp" && mv "$tmp" "$json_file"
  fi
}

orch_progress_set() {
  local id="$1" phase="$2"
  shift 2
  local msg="$*"
  local dir status_file
  dir="$(orch_progress_dir)"
  mkdir -p "$dir"
  status_file="$dir/${id}.status"
  printf '%s|%s\n' "$phase" "$msg" >"$status_file"
  orch_progress_write_json "$id" "$phase" "$msg"
  orch_progress_tmux_state "$id" "$phase"
  orch_cdamux_publish_worker "$id" "$phase" "$msg"

  if ! orch_progress_enabled; then
    printf '[#%s] %s: %s\n' "$id" "$phase" "$msg" >&2
  fi
}

# Record what worker $1 runs (ORCH_AGENT_CLI/MODEL/EFFORT/TIER, set by orch_agent_run)
# in its json and status.json; phase and message stay as they are. Empty values → null.
orch_progress_agent() {
  local id="$1" line=""
  command -v jq >/dev/null 2>&1 || return 0
  [ -f "$(orch_progress_dir)/${id}.status" ] && line="$(cat "$(orch_progress_dir)/${id}.status")"
  [ -n "$line" ] || line="agent|starting"
  orch_progress_write_json "$id" "${line%%|*}" "${line#*|}" "$(jq -cn \
    --arg cli "${ORCH_AGENT_CLI:-}" --arg model "${ORCH_AGENT_MODEL:-}" \
    --arg effort "${ORCH_AGENT_EFFORT:-}" --arg tier "${ORCH_AGENT_TIER:-}" \
    '{cli:$cli, model:$model, effort:$effort, tier:$tier} | map_values(if . == "" then null else . end)')"
}

# ORCH_TMUX=1: mirror the phase as @cdamux_state on the worker's session; no-op without one.
orch_progress_tmux_state() {
  local s state
  declare -F orch_tmux_on >/dev/null && orch_tmux_on || return 0
  s="$(orch_tmux_session "$1")"
  tmux has-session -t "=$s:" 2>/dev/null || return 0
  case "$2" in
    done) state=done ;;
    blocked) state=error ;;
    pending) state=waiting ;;
    *) state=running ;;
  esac
  tmux set-option -t "=$s:" @cdamux_state "$state" 2>/dev/null || true
}

orch_progress_icon() {
  local phase="$1" frame="$2"
  case "$phase" in
    done) printf '✓';;
    blocked) printf '✗';;
    pending) printf '·';;
    *) printf '%s' "$frame";;
  esac
}

orch_progress_draw() {
  local dir frame i line id phase msg icon ids=()
  dir="$(orch_progress_dir)"
  [ -d "$dir" ] || return 0

  # Collect known worker ids (status files + explicit list)
  if [ -n "${ORCH_PROGRESS_IDS:-}" ]; then
    # shellcheck disable=SC2206
    ids=( $ORCH_PROGRESS_IDS )
  else
    local f
    for f in "$dir"/*.status; do
      [ -e "$f" ] || continue
      ids+=("$(basename "$f" .status)")
    done
  fi
  [ "${#ids[@]}" -gt 0 ] || return 0

  frame="${ORCH_SPINNER_FRAMES:0:1}"
  i=$(( ${ORCH_SPINNER_I:-0} % ${#ORCH_SPINNER_FRAMES} ))
  frame="${ORCH_SPINNER_FRAMES:$i:1}"

  # Move to dashboard home and clear downward
  if [ -n "${ORCH_PROGRESS_ROWS:-}" ]; then
    printf '\033[%sA' "$ORCH_PROGRESS_ROWS" >&2
  fi
  printf '\033[J' >&2

  printf '┌─ orch workers ──────────────────────────────────────┐\n' >&2
  local rows=1
  for id in "${ids[@]}"; do
    phase="pending"
    msg="queued"
    if [ -f "$dir/${id}.status" ]; then
      line="$(cat "$dir/${id}.status")"
      phase="${line%%|*}"
      msg="${line#*|}"
    fi
    icon="$(orch_progress_icon "$phase" "$frame")"
    printf '│ %s #%-6s %-8s %-36s │\n' "$icon" "$id" "$phase" "${msg:0:36}" >&2
    rows=$((rows + 1))
  done
  printf '└─────────────────────────────────────────────────────┘\n' >&2
  rows=$((rows + 1))
  ORCH_PROGRESS_ROWS="$rows"
  ORCH_SPINNER_I=$(( ${ORCH_SPINNER_I:-0} + 1 ))
}

orch_progress_dashboard_loop() {
  export ORCH_SPINNER_FRAMES='⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏'
  export ORCH_SPINNER_I=0
  export ORCH_PROGRESS_ROWS=""
  # Initial blank board
  orch_progress_draw
  while [ ! -f "$(orch_progress_dir)/.dashboard-stop" ]; do
    sleep 0.12
    orch_progress_draw
  done
  # Final draw (settle icons)
  orch_progress_draw
  # Leave cursor below the board
  ORCH_PROGRESS_ROWS=""
}

orch_progress_dashboard_start() {
  orch_progress_enabled || return 0
  local dir
  dir="$(orch_progress_dir)"
  mkdir -p "$dir"
  rm -f "$dir/.dashboard-stop"
  export ORCH_PROGRESS_IDS="${ORCH_PROGRESS_IDS:-}"
  orch_progress_dashboard_loop &
  ORCH_PROGRESS_PID=$!
  export ORCH_PROGRESS_PID
}

orch_progress_dashboard_stop() {
  local dir pid
  dir="$(orch_progress_dir)"
  mkdir -p "$dir"
  : >"$dir/.dashboard-stop"
  pid="${ORCH_PROGRESS_PID:-}"
  if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
    wait "$pid" 2>/dev/null || true
  fi
  unset ORCH_PROGRESS_PID
  # One newline after board for following summary lines
  printf '\n' >&2
}
