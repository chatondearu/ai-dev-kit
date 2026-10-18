#!/usr/bin/env bash
# orch-auto.sh — discover Ready Kanban issues and run ready-pickup (ADR-002).
# Opt-in AFK poller; pair with templates/systemd/ for start/stop.
#
#   bash orch-auto.sh --repo DIR [--once] [--dry-run]
#   bash orch-auto.sh --help
set -euo pipefail
_self="${BASH_SOURCE[0]:-$0}"
_dir="${_self%/*}"
[ "$_dir" = "$_self" ] && _dir="."
ROOT="$(cd "$_dir" && pwd)"

REPO="$(pwd)"
DRY=0
ONCE=0
MAX_P="${ORCH_MAX_PARALLEL:-1}"

usage() {
  cat <<'EOF'
orch-auto.sh — poll Ready cards → orch ready-pickup

  --repo DIR     working tree with board (default: cwd)
  --once         one discovery+dispatch cycle (default for systemd oneshot)
  --dry-run      print eligible issue numbers; do not invoke orch
  --max-parallel N   passed to orch (default: ORCH_MAX_PARALLEL or 1)
  -h|--help

Skips: triage:ready-for-human, issues with an open PR on feat/<id>-issue-<id>.
Uses flock on <repo>/.orch/auto.lock. Logs under <repo>/.orch/auto/.
EOF
}

while [ $# -gt 0 ]; do
  case "$1" in
    --repo) REPO="$2"; shift;;
    --once) ONCE=1;;
    --dry-run) DRY=1;;
    --max-parallel) MAX_P="$2"; shift;;
    -h|--help) usage; exit 0;;
    *) echo "unknown: $1" >&2; usage >&2; exit 2;;
  esac
  shift
done

REPO="$(cd "$REPO" && pwd)"
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
LOG_FILE="$LOG_DIR/$(date -u +%Y%m%dT%H%M%SZ).log"

log() {
  printf '%s\n' "$*" | tee -a "$LOG_FILE" >&2
}

# True if issue has label triage:ready-for-human.
issue_ready_for_human() {
  local id="$1"
  local labels raw
  raw="$(gh issue view "$id" --repo "$OWNER/$REPO_NAME" --json labels 2>/dev/null || true)"
  labels="$(printf '%s' "${raw:-{\"labels\":[]}}" | jq -r '[.labels[].name] | join(",")' 2>/dev/null || true)"
  [[ ",$labels," == *",triage:ready-for-human,"* ]]
}

# True if an open PR already targets the expected ready-pickup branch.
issue_has_open_pr() {
  local id="$1"
  local branch="feat/${id}-issue-${id}"
  local json
  json="$(gh pr list --repo "$OWNER/$REPO_NAME" --head "$branch" --state open \
    --json number 2>/dev/null || echo '[]')"
  [ "$(printf '%s' "$json" | jq 'length')" -gt 0 ]
}

discover_eligible() {
  local nums n
  nums="$(bash "$GH_BOARD" list-ready --repo "$OWNER/$REPO_NAME" 2>/dev/null)" || {
    log "orch-auto: list-ready failed"
    return 1
  }
  ELIGIBLE=()
  while IFS= read -r n; do
    [ -n "$n" ] || continue
    if issue_ready_for_human "$n"; then
      log "skip #$n (ready-for-human)"
      continue
    fi
    if issue_has_open_pr "$n"; then
      log "skip #$n (open PR on feat/${n}-issue-${n})"
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
  echo "orch-auto: another run holds $LOCK" >&2
  exit 0
fi

if [ "$ONCE" = 1 ] || [ "${ORCH_AUTO_ONCE:-0}" = 1 ]; then
  run_once
  exit $?
fi

# Interactive / long-running: one cycle then exit (timer should use --once).
run_once
