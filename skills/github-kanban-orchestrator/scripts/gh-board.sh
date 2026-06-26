#!/usr/bin/env bash
# gh-board.sh — resolve GitHub Projects (v2) identifiers and listen to board events.
#
# No hardcoded IDs: owner, repo, project number, field IDs and single-select
# option IDs are all discovered at runtime via `gh` and `gh api graphql`.
#
# Usage:
#   ./gh-board.sh env [--owner OWNER] [--repo OWNER/REPO] [--project-number N]
#   eval "$(./gh-board.sh env)"            # export everything into the shell
#   ./gh-board.sh item-id <ISSUE_NUMBER>   # resolve a board item id from issue #
#   ./gh-board.sh events [--since ISO|--since-file PATH] [--json] [--limit N]
#   ./gh-board.sh watch [--interval SECONDS] [--limit N] [--json]
#
# `events` aggregates, per board item and since a cursor, a normalized activity
# feed: comments, reviews, labels, assignment, PR merge/close, and Status column
# changes (detected via snapshot diff — Projects v2 has no field-history API).
#
# Requires: gh (with `project` scope), jq.
set -euo pipefail

err() { printf 'gh-board: %s\n' "$*" >&2; }
die() { err "$*"; exit 1; }

need() { command -v "$1" >/dev/null 2>&1 || die "missing dependency: $1"; }

# Temp-file cleanup (set -u safe), runs on exit.
_CLEANUP=()
cleanup() { local f; for f in "${_CLEANUP[@]:-}"; do [ -n "$f" ] && rm -f "$f"; done; }
trap cleanup EXIT

now_iso() { date -u +%Y-%m-%dT%H:%M:%SZ; }

# 24h ago, ISO-8601 UTC. Tries GNU date, falls back to BSD/macOS date.
since_default() {
  date -u -d '24 hours ago' +%Y-%m-%dT%H:%M:%SZ 2>/dev/null \
    || date -u -v-24H +%Y-%m-%dT%H:%M:%SZ
}

repo_root() { git rev-parse --show-toplevel 2>/dev/null || pwd; }

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

# Emit normalized event objects (JSONL) from one issue/PR timeline since $SINCE.
emit_timeline() {
  local n="$1" title="$2" url="$3"
  gh api --paginate "repos/$OWNER/$REPO/issues/$n/timeline?per_page=100" 2>/dev/null \
    | jq -c --arg since "$SINCE" --argjson n "$n" --arg title "$title" --arg url "$url" '
        .[]
        | select(.created_at != null and .created_at >= $since)
        | . as $e
        | (
            if .event=="commented" then
              {type:"comment", actor:(.user.login // "?"), detail:((.body // "")[0:100]), url:(.html_url // $url)}
            elif .event=="reviewed" then
              {type:("review_" + ((.state // "") | ascii_downcase)), actor:(.user.login // "?"), detail:((.body // "")[0:100]), url:(.html_url // $url)}
            elif .event=="labeled" then
              {type:"labeled", actor:(.actor.login // "?"), detail:("+label " + (.label.name // "")), url:$url}
            elif .event=="unlabeled" then
              {type:"unlabeled", actor:(.actor.login // "?"), detail:("-label " + (.label.name // "")), url:$url}
            elif .event=="assigned" then
              {type:"assigned", actor:(.actor.login // "?"), detail:("assignee " + (.assignee.login // "")), url:$url}
            elif .event=="unassigned" then
              {type:"unassigned", actor:(.actor.login // "?"), detail:("assignee " + (.assignee.login // "")), url:$url}
            elif .event=="merged" then
              {type:"pr_merged", actor:(.actor.login // "?"), detail:"merged", url:$url}
            elif .event=="closed" then
              {type:"closed", actor:(.actor.login // "?"), detail:"closed", url:$url}
            else empty end
          )
        | . + {ts:$e.created_at, number:$n, title:$title}
      '
}

# Diff current board Status per item against the stored snapshot; emit
# status_changed events; persist the new snapshot.
emit_status_changes() {
  local items_file="$1" snap="$2" cur ts
  ts="$(now_iso)"
  cur="$(jq -c -s '
    map({key:(.number|tostring), value:{status:.status, title:.title, url:.url}})
    | from_entries' "$items_file")"
  if [ -f "$snap" ]; then
    jq -c -n --slurpfile prevArr "$snap" --argjson cur "$cur" --arg ts "$ts" '
      ($prevArr[0] // {}) as $prev
      | $cur | to_entries[]
      | . as $e
      | ($prev[$e.key].status // null) as $old
      | select($old != null and $old != $e.value.status)
      | {ts:$ts, type:"status_changed", actor:"?", number:($e.key|tonumber),
         title:$e.value.title, url:$e.value.url,
         detail:(($old // "∅") + " → " + ($e.value.status // "∅"))}'
  fi
  printf '%s\n' "$cur" > "$snap"
}

cmd_events() {
  need gh; need jq
  resolve_repo
  resolve_project_number

  local state_dir since_file snap
  state_dir="$(repo_root)/.tmp/kanban"
  mkdir -p "$state_dir"
  snap="$state_dir/${OWNER}-${REPO}.status.json"

  # Resolve cursor (SINCE) and whether to persist it.
  local use_cursor=1
  if [ -n "${OVR_SINCE:-}" ]; then
    SINCE="$OVR_SINCE"; use_cursor=0
  else
    since_file="${OVR_SINCEFILE:-$state_dir/${OWNER}-${REPO}.cursor}"
    if [ -s "$since_file" ]; then SINCE="$(cat "$since_file")"; else SINCE="$(since_default)"; fi
  fi

  local run_start; run_start="$(now_iso)"

  # Board items in THIS repo (number, title, url, status).
  local items_file events_file
  items_file="$(mktemp)"; events_file="$(mktemp)"
  _CLEANUP+=("$items_file" "$events_file")

  gh project item-list "$PROJECT_NUMBER" --owner "$OWNER" --format json --limit "$LIMIT" \
    | jq -c --arg slug "$OWNER/$REPO" '
        .items[]
        | select(.content.url != null and (.content.url | contains("/" + $slug + "/")))
        | {number:.content.number, title:(.content.title // ""),
           url:.content.url, status:(.status // "")}' \
    > "$items_file" || die "cannot list project items"

  # Timeline events per item.
  while IFS= read -r line; do
    [ -z "$line" ] && continue
    local n title url
    n="$(printf '%s' "$line" | jq -r '.number')"
    title="$(printf '%s' "$line" | jq -r '.title')"
    url="$(printf '%s' "$line" | jq -r '.url')"
    emit_timeline "$n" "$title" "$url" >> "$events_file" || true
  done < "$items_file"

  # Status column changes (snapshot diff).
  emit_status_changes "$items_file" "$snap" >> "$events_file" || true

  # Render sorted feed.
  if [ "${OUT_JSON:-0}" = "1" ]; then
    jq -s 'sort_by(.ts)' "$events_file"
  else
    if [ ! -s "$events_file" ]; then
      printf 'no new events since %s\n' "$SINCE"
    else
      jq -s -r 'sort_by(.ts)[]
        | "\(.ts)  \(.type|ascii_upcase)  #\(.number)  @\(.actor)  \(.detail)  \(.url)"' \
        "$events_file"
    fi
  fi

  # Advance the cursor so the next run only sees newer events.
  if [ "$use_cursor" = "1" ]; then printf '%s\n' "$run_start" > "$since_file"; fi
}

cmd_watch() {
  local interval="${OVR_INTERVAL:-60}"
  err "watching board events every ${interval}s (Ctrl-C to stop)"
  while true; do
    printf '── poll %s ──\n' "$(now_iso)"
    cmd_events || err "poll failed, retrying next cycle"
    sleep "$interval"
  done
}

main() {
  local sub="${1:-env}"; shift || true
  OVR_OWNER=""; OVR_REPO=""; OVR_PNUM=""
  OVR_SINCE=""; OVR_SINCEFILE=""; OVR_INTERVAL=""
  OUT_JSON=0; LIMIT=100
  local positional=()
  while [ $# -gt 0 ]; do
    case "$1" in
      --owner) OVR_OWNER="$2"; shift 2;;
      --repo) OVR_REPO="$2"; shift 2;;
      --project-number) OVR_PNUM="$2"; shift 2;;
      --since) OVR_SINCE="$2"; shift 2;;
      --since-file) OVR_SINCEFILE="$2"; shift 2;;
      --interval) OVR_INTERVAL="$2"; shift 2;;
      --limit) LIMIT="$2"; shift 2;;
      --json) OUT_JSON=1; shift;;
      *) positional+=("$1"); shift;;
    esac
  done
  case "$sub" in
    env) cmd_env;;
    item-id) cmd_item_id "${positional[0]:-}";;
    events) cmd_events;;
    watch) cmd_watch;;
    *) die "unknown subcommand: $sub (use: env | item-id | events | watch)";;
  esac
}

main "$@"
