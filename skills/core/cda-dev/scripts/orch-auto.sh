#!/usr/bin/env bash
# orch-auto.sh — discover Ready Kanban issues and run ready-pickup (ADR-002).
# Opt-in AFK poller; pair with templates/systemd/ for start/stop.
#
#   bash orch-auto.sh --repo DIR [--dry-run]
#   bash orch-auto.sh --help
set -euo pipefail
_self="${BASH_SOURCE[0]:-$0}"
_dir="${_self%/*}"
[ "$_dir" = "$_self" ] && _dir="."
ROOT="$(cd "$_dir" && pwd)"

REPO="$(pwd)"
DRY=0
MAX_P="${ORCH_MAX_PARALLEL:-1}"

usage() {
  cat <<'EOF'
orch-auto.sh — poll Ready cards → orch ready-pickup

  --repo DIR     working tree with board (default: cwd)
  --dry-run      print eligible issue numbers; do not invoke orch
  --max-parallel N   passed to orch (default: ORCH_MAX_PARALLEL or 1)
  -h|--help

Skips: triage:ready-for-human, issues with an open PR on feat/<id>-issue-<id>,
and (fail closed) any issue whose label or PR lookup fails.
Runs one discovery+dispatch cycle, then exits (systemd oneshot).
Uses flock on <repo>/.orch/auto.lock. Logs under <repo>/.orch/auto/ (newest 50 kept).
EOF
}

while [ $# -gt 0 ]; do
  case "$1" in
    --repo) REPO="$2"; shift;;
    --dry-run) DRY=1;;
    --max-parallel) MAX_P="$2"; shift;;
    -h|--help) usage; exit 0;;
    *) echo "unknown: $1" >&2; usage >&2; exit 2;;
  esac
  shift
done

source "$ROOT/lib/worktree.sh"
REPO="$(orch_repo_root "$REPO")"
export ORCH_REPO="$REPO" ORCH_PROGRESS="${ORCH_PROGRESS:-0}"

GH_BOARD="${ORCH_GH_BOARD:-}"
if [ -z "$GH_BOARD" ]; then
  for cand in \
    "$ROOT/../../cda-kanban/scripts/gh-board.sh" \
    "${HOME}/.cursor/skills/cda-kanban/scripts/gh-board.sh" \
    "${HOME}/.claude/skills/cda-kanban/scripts/gh-board.sh"
  do
    if [ -f "$cand" ]; then
      GH_BOARD="$(cd "$(dirname "$cand")" && pwd)/$(basename "$cand")"
      break
    fi
  done
fi
[ -n "$GH_BOARD" ] && [ -f "$GH_BOARD" ] || {
  echo "orch-auto: gh-board.sh not found (install cda-kanban skill)" >&2
  exit 1
}

need() { command -v "$1" >/dev/null 2>&1 || { echo "orch-auto: missing $1" >&2; exit 1; }; }
need gh
need jq
need flock

LOG_DIR="$REPO/.orch/auto"
mkdir -p "$LOG_DIR" "$REPO/.orch"
LOCK="$REPO/.orch/auto.lock"
# Keep the newest 49 logs plus the one created below.
ls -1t "$LOG_DIR"/*.log 2>/dev/null | tail -n +50 | while IFS= read -r old; do rm -f "$old"; done || true
LOG_FILE="$LOG_DIR/$(date -u +%Y%m%dT%H%M%SZ).log"

log() {
  printf '%s\n' "$*" | tee -a "$LOG_FILE" >&2
}

# Print why issue $1 must be skipped and return 0; return 1 when eligible.
# Fail closed: a gh/jq failure (rate limit, network) skips the issue, like frontier.
issue_skip_reason() {
  local id="$1" branch="feat/${1}-issue-${1}" raw labels n
  if ! raw="$(gh issue view "$id" --repo "$OWNER/$REPO_NAME" --json labels 2>/dev/null)" ||
    ! labels="$(printf '%s' "$raw" | jq -er '[.labels[].name] | join(",")' 2>/dev/null)"; then
    echo "labels lookup failed; fail closed"
    return 0
  fi
  if [[ ",$labels," == *",triage:ready-for-human,"* ]]; then
    echo "ready-for-human"
    return 0
  fi
  if ! raw="$(gh pr list --repo "$OWNER/$REPO_NAME" --head "$branch" --state open \
      --json number 2>/dev/null)" ||
    ! n="$(printf '%s' "$raw" | jq -e 'length' 2>/dev/null)"; then
    echo "open-PR lookup failed; fail closed"
    return 0
  fi
  if [ "$n" -gt 0 ]; then
    echo "open PR on $branch"
    return 0
  fi
  return 1
}

discover_eligible() {
  local nums n reason
  nums="$(bash "$GH_BOARD" list-ready --repo "$OWNER/$REPO_NAME" 2>/dev/null)" || {
    log "orch-auto: list-ready failed"
    return 1
  }
  ELIGIBLE=()
  while IFS= read -r n; do
    [ -n "$n" ] || continue
    if reason="$(issue_skip_reason "$n")"; then
      log "skip #$n ($reason)"
      continue
    fi
    ELIGIBLE+=("$n")
  done <<< "$nums"
}

run_once() {
  local owner_repo
  owner_repo="$(gh repo view --json nameWithOwner 2>/dev/null | jq -r .nameWithOwner 2>/dev/null || true)"
  if [ -z "$owner_repo" ] || [ "$owner_repo" = "null" ]; then
    log "orch-auto: not a GitHub repo (or gh not authed)"
    return 1
  fi
  OWNER="${owner_repo%%/*}"
  REPO_NAME="${owner_repo##*/}"

  ELIGIBLE=()
  discover_eligible || return 1

  if [ "${#ELIGIBLE[@]}" -eq 0 ]; then
    log "orch-auto: no eligible Ready issues"
    return 0
  fi

  log "orch-auto: eligible ${ELIGIBLE[*]}"

  if [ "$DRY" = 1 ]; then
    printf '%s\n' "${ELIGIBLE[@]}"
    return 0
  fi

  local args=(--repo "$REPO" --mode ready-pickup --max-parallel "$MAX_P" --no-progress)
  local id
  for id in "${ELIGIBLE[@]}"; do
    args+=(--issue "$id")
  done

  log "orch-auto: invoking orch.sh ${args[*]}"
  bash "$ROOT/orch.sh" "${args[@]}"
}

# Resolve owner/repo from cwd of REPO for list-ready --repo flag.
cd "$REPO"

exec 9>"$LOCK"
if ! flock -n 9; then
  echo "orch-auto: another run holds $LOCK; exiting 75 (EX_TEMPFAIL)" >&2
  exit 75
fi

run_once
