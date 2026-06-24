#!/usr/bin/env bash
# gh-board.sh — resolve GitHub Projects (v2) identifiers for the current repo.
#
# No hardcoded IDs: owner, repo, project number, field IDs and single-select
# option IDs are all discovered at runtime via `gh` and `gh api graphql`.
#
# Usage:
#   ./gh-board.sh env [--owner OWNER] [--repo OWNER/REPO] [--project-number N]
#   eval "$(./gh-board.sh env)"            # export everything into the shell
#   ./gh-board.sh item-id <ISSUE_NUMBER>   # resolve a board item id from issue #
#
# Requires: gh (with `project` scope), jq.
set -euo pipefail

err() { printf 'gh-board: %s\n' "$*" >&2; }
die() { err "$*"; exit 1; }

need() { command -v "$1" >/dev/null 2>&1 || die "missing dependency: $1"; }

# Normalize a label into a shell-safe SUFFIX: "In progress" -> IN_PROGRESS.
norm() {
  printf '%s' "$1" \
    | tr '[:lower:]' '[:upper:]' \
    | sed -E 's/[^A-Z0-9]+/_/g; s/^_+//; s/_+$//'
}

resolve_repo() {
  # Sets OWNER, REPO. Honors --repo / --owner overrides via env OVR_*.
  if [ -n "${OVR_REPO:-}" ]; then
    OWNER="${OVR_REPO%%/*}"; REPO="${OVR_REPO##*/}"; return
  fi
  local json; json="$(gh repo view --json owner,name 2>/dev/null)" \
    || die "not in a GitHub repo (or gh not authenticated)"
  OWNER="$(printf '%s' "$json" | jq -r '.owner.login')"
  REPO="$(printf '%s' "$json" | jq -r '.name')"
  if [ -n "${OVR_OWNER:-}" ]; then OWNER="$OVR_OWNER"; fi
}

resolve_project_number() {
  # Sets PROJECT_NUMBER. Override wins; else first project linked to the repo;
  # else fail with guidance.
  if [ -n "${OVR_PNUM:-}" ]; then PROJECT_NUMBER="$OVR_PNUM"; return; fi
  local nodes count
  nodes="$(gh api graphql -f query='
    query($owner:String!, $name:String!) {
      repository(owner:$owner, name:$name) {
        projectsV2(first:20) { nodes { number title } }
      }
    }' -F owner="$OWNER" -F name="$REPO" \
    --jq '.data.repository.projectsV2.nodes' 2>/dev/null)" \
    || die "failed to query linked projects (need 'project' scope: gh auth refresh -s project)"
  count="$(printf '%s' "$nodes" | jq 'length')"
  if [ "$count" = "0" ]; then
    die "no GitHub Project linked to $OWNER/$REPO — create+link one (see SKILL.md 'Bootstrap a board')"
  elif [ "$count" = "1" ]; then
    PROJECT_NUMBER="$(printf '%s' "$nodes" | jq -r '.[0].number')"
  else
    err "multiple linked projects found:"
    printf '%s' "$nodes" | jq -r '.[] | "  #\(.number) \(.title)"' >&2
    die "pass --project-number N to choose"
  fi
}

cmd_env() {
  need gh; need jq
  resolve_repo
  resolve_project_number

  local fields
  fields="$(gh project field-list "$PROJECT_NUMBER" --owner "$OWNER" --format json 2>/dev/null)" \
    || die "cannot read fields for project #$PROJECT_NUMBER (owner $OWNER)"

  PROJECT_ID="$(gh project view "$PROJECT_NUMBER" --owner "$OWNER" --format json --jq '.id')"

  printf 'export OWNER=%q\n' "$OWNER"
  printf 'export REPO=%q\n' "$REPO"
  printf 'export PROJECT_NUMBER=%q\n' "$PROJECT_NUMBER"
  printf 'export PROJECT_ID=%q\n' "$PROJECT_ID"

  # For every single-select field, export <FIELD>_FIELD_ID and <FIELD>_<OPTION>.
  local rows
  rows="$(printf '%s' "$fields" | jq -r '
    .fields[]
    | select(.options != null)
    | . as $f
    | ($f.name) as $fname
    | "FIELD\t\($fname)\t\($f.id)",
      ( $f.options[] | "OPT\t\($fname)\t\(.name)\t\(.id)" )
  ')"

  while IFS=$'\t' read -r kind a b c; do
    case "$kind" in
      FIELD)
        printf 'export %s_FIELD_ID=%q\n' "$(norm "$a")" "$b"
        ;;
      OPT)
        printf 'export %s_%s=%q\n' "$(norm "$a")" "$(norm "$b")" "$c"
        ;;
    esac
  done <<< "$rows"
}

cmd_item_id() {
  need gh; need jq
  local issue="${1:?usage: gh-board.sh item-id <ISSUE_NUMBER>}"
  resolve_repo
  resolve_project_number
  gh project item-list "$PROJECT_NUMBER" --owner "$OWNER" --format json --limit 200 \
    | jq -r --argjson n "$issue" \
        '.items[] | select(.content.number == $n) | .id'
}

main() {
  local sub="${1:-env}"; shift || true
  OVR_OWNER=""; OVR_REPO=""; OVR_PNUM=""
  local positional=()
  while [ $# -gt 0 ]; do
    case "$1" in
      --owner) OVR_OWNER="$2"; shift 2;;
      --repo) OVR_REPO="$2"; shift 2;;
      --project-number) OVR_PNUM="$2"; shift 2;;
      *) positional+=("$1"); shift;;
    esac
  done
  case "$sub" in
    env) cmd_env;;
    item-id) cmd_item_id "${positional[0]:-}";;
    *) die "unknown subcommand: $sub (use: env | item-id)";;
  esac
}

main "$@"
