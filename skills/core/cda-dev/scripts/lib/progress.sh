#!/usr/bin/env bash
# shellcheck shell=bash
# Source-only: worker status files + optional TTY progress dashboard.

# Status line format: PHASE|message  (PHASE = pending|worktree|agent|verify|ci|done|blocked)

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
  local dir ts json_file agg
  dir="$(orch_progress_dir)"
  mkdir -p "$dir"
  ts="$(date -u +"%Y-%m-%dT%H:%M:%SZ" 2>/dev/null || date -u)"
  json_file="$dir/${id}.json"
  if command -v jq >/dev/null 2>&1; then
    jq -n \
      --arg id "$id" \
      --arg phase "$phase" \
      --arg message "$msg" \
      --arg updated_at "$ts" \
      '{id:$id, phase:$phase, message:$message, updated_at:$updated_at}' \
      >"$json_file"
    # Aggregate all workers into one file agents can poll.
    agg="$dir/status.json"
    local files=() f
    for f in "$dir"/*.json; do
      [ -e "$f" ] || continue
      case "$f" in */status.json) continue ;; esac
      files+=("$f")
    done
    if [ "${#files[@]}" -eq 0 ]; then
      jq -n --arg ts "$ts" '{updated_at:$ts, workers:[]}' >"$agg"
    else
      jq -s --arg ts "$ts" '{updated_at:$ts, workers:.}' "${files[@]}" >"$agg"
    fi
  else
    printf '{"id":"%s","phase":"%s","message":"%s","updated_at":"%s"}\n' \
      "$id" "$phase" "$msg" "$ts" >"$json_file"
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

  if ! orch_progress_enabled; then
    printf '[#%s] %s: %s\n' "$id" "$phase" "$msg" >&2
  fi
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
