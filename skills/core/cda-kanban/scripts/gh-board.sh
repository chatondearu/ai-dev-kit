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
#   ./gh-board.sh reconcile-backlog [--owner O] [--repo O/R] [--project-number N]
#     [--base origin/dev] [--status Backlog] [--apply] [--json] [--limit 100]
#     [--chunk 40]
#   ./gh-board.sh list-ready [--json] [--limit N]
#     Ready-column open Issues (numbers); used by orch-auto.sh
#
# `events` aggregates, per board item and since a cursor, a normalized activity
# feed: comments, reviews, labels, assignment, PR merge/close, and Status column
# changes (detected via snapshot diff — Projects v2 has no field-history API).
#
# `reconcile-backlog` moves cards from Backlog (or --status) to Done when local
# git / issue state already proves the work landed — API-efficient (1 list query
# + ceil(N/chunk) aliased mutations). Dry-run unless --apply. Status only;
# never force-closes issues. See: reconcile-backlog --help
#
# Requires: gh (with `project` scope), jq.
set -euo pipefail

err() { printf 'gh-board: %s\n' "$*" >&2; }
die() { err "$*"; exit 1; }

need() { command -v "$1" >/dev/null 2>&1 || die "missing dependency: $1"; }

# Temp-file cleanup (set -u safe), runs on exit.
# Use `if` not `&&` so an empty slot cannot flip the script exit status under set -e.
_CLEANUP=()
cleanup() {
  local f
  for f in "${_CLEANUP[@]+"${_CLEANUP[@]}"}"; do
    if [ -n "$f" ]; then rm -f "$f"; fi
  done
}
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

# Print Ready-column open Issues (one number per line, or JSON with --json).
cmd_list_ready() {
  need gh; need jq
  resolve_status_board
  local ready_line ready_name items
  ready_line="$(closest_status_option "Ready" "$STATUS_OPTIONS")"
  [ -n "$ready_line" ] || die "no Status option matching Ready"
  ready_name="$(printf '%s' "$ready_line" | cut -f2)"
  items="$(list_project_items_graphql)"
  if [ "$OUT_JSON" = 1 ]; then
    printf '%s' "$items" | jq -c --arg st "$ready_name" '
      [.[] | select(.status == $st
                    and .typename == "Issue"
                    and (.state | ascii_upcase) == "OPEN")
       | {number, title, status}]'
  else
    printf '%s' "$items" | jq -r --arg st "$ready_name" '
      .[] | select(.status == $st
                   and .typename == "Issue"
                   and (.state | ascii_upcase) == "OPEN")
      | .number'
  fi
}

# Map intent name to an existing Status option id (case-insensitive exact match only).
# Soft substring match was removed: "Done" must not match "Abandoned", "Ready"
# must not match "Already Reviewed". Prints: <optionId>\t<optionName> or empty.
closest_status_option() {
  local want="$1" options_json="$2"
  printf '%s' "$options_json" | jq -r --arg w "$want" '
    ($w | ascii_downcase) as $needle
    | (map(select((.name | ascii_downcase) == $needle)) | .[0]) as $exact
    | if $exact then "\($exact.id)\t\($exact.name)" else empty end
  '
}

# Resolve PROJECT_ID + Status field/options (same sources as cmd_env).
resolve_status_board() {
  resolve_repo
  resolve_project_number
  PROJECT_ID="$(gh project view "$PROJECT_NUMBER" --owner "$OWNER" --format json --jq '.id')" \
    || die "cannot view project #$PROJECT_NUMBER"
  local fields
  fields="$(gh project field-list "$PROJECT_NUMBER" --owner "$OWNER" --format json 2>/dev/null)" \
    || die "cannot read fields for project #$PROJECT_NUMBER"
  STATUS_FIELD_ID="$(printf '%s' "$fields" | jq -r '
    .fields[] | select(.name=="Status" and .options != null) | .id' | head -1)"
  [ -n "$STATUS_FIELD_ID" ] && [ "$STATUS_FIELD_ID" != "null" ] \
    || die "no Status single-select field on project #$PROJECT_NUMBER"
  STATUS_OPTIONS="$(printf '%s' "$fields" | jq -c '
    [.fields[] | select(.name=="Status" and .options != null) | .options[]]')"
}

resolve_base_ref() {
  # Sets BASE_REF. Default origin/dev; fallback to remote default branch.
  local want="${OVR_BASE:-origin/dev}"
  git fetch origin --quiet 2>/dev/null || true
  if git rev-parse --verify "$want" >/dev/null 2>&1; then
    BASE_REF="$want"; return
  fi
  local def
  def="$(git symbolic-ref -q refs/remotes/origin/HEAD 2>/dev/null | sed 's@^refs/remotes/@@')"
  if [ -z "$def" ]; then
    def="origin/$(gh repo view --json defaultBranchRef --jq '.defaultBranchRef.name' 2>/dev/null || echo main)"
  fi
  err "base '$want' missing; falling back to $def"
  git rev-parse --verify "$def" >/dev/null 2>&1 \
    || die "cannot resolve git base (tried $want and $def)"
  BASE_REF="$def"
}

# Strong local signal: commit on BASE_REF mentions #N / Closes #N / Fixes #N.
git_mentions_issue() {
  local n="$1" hit
  hit="$(git log "$BASE_REF" --extended-regexp -i \
    --grep="(Closes|Fixes|Resolves|Refs)[[:space:]]+#${n}([^0-9]|$)" \
    --grep="#${n}([^0-9]|$)" \
    --pretty=format:'%h' -n 1 2>/dev/null || true)"
  [ -n "$hit" ]
}

# One paginated GraphQL list of project items (issue/PR + Status). Honors LIMIT.
list_project_items_graphql() {
  local after="" collected=0 page_size=100 chunk slug="$OWNER/$REPO"
  local out; out="$(mktemp)"; _CLEANUP+=("$out")
  printf '[]' > "$out"

  while true; do
    local remaining=$((LIMIT - collected))
    [ "$remaining" -gt 0 ] || break
    page_size=100
    [ "$remaining" -lt "$page_size" ] && page_size="$remaining"

    local page
    if [ -n "$after" ]; then
      page="$(gh api graphql -f query='
        query($id:ID!, $first:Int!, $after:String!) {
          node(id:$id) {
            ... on ProjectV2 {
              items(first:$first, after:$after) {
                pageInfo { hasNextPage endCursor }
                nodes {
                  id
                  fieldValueByName(name:"Status") {
                    ... on ProjectV2ItemFieldSingleSelectValue { name }
                  }
                  content {
                    __typename
                    ... on Issue {
                      number title state closedAt
                      repository { nameWithOwner }
                    }
                    ... on PullRequest {
                      number title state closedAt merged
                      repository { nameWithOwner }
                    }
                  }
                }
              }
            }
          }
        }' -f id="$PROJECT_ID" -F first="$page_size" -f after="$after" 2>/dev/null)" \
        || die "GraphQL project items query failed (need project scope)"
    else
      page="$(gh api graphql -f query='
        query($id:ID!, $first:Int!) {
          node(id:$id) {
            ... on ProjectV2 {
              items(first:$first) {
                pageInfo { hasNextPage endCursor }
                nodes {
                  id
                  fieldValueByName(name:"Status") {
                    ... on ProjectV2ItemFieldSingleSelectValue { name }
                  }
                  content {
                    __typename
                    ... on Issue {
                      number title state closedAt
                      repository { nameWithOwner }
                    }
                    ... on PullRequest {
                      number title state closedAt merged
                      repository { nameWithOwner }
                    }
                  }
                }
              }
            }
          }
        }' -f id="$PROJECT_ID" -F first="$page_size" 2>/dev/null)" \
        || die "GraphQL project items query failed (need project scope)"
    fi

    local page_n
    page_n="$(printf '%s' "$page" | jq '.data.node.items.nodes | length')"
    collected=$((collected + page_n))

    chunk="$(printf '%s' "$page" | jq -c --arg slug "$slug" '
      [.data.node.items.nodes[]?
       | select(.content != null
                and .content.number != null
                and (.content.repository.nameWithOwner // "") == $slug)
       | {
           itemId: .id,
           number: .content.number,
           title: (.content.title // ""),
           state: (.content.state // ""),
           closedAt: (.content.closedAt // null),
           merged: (.content.merged // false),
           typename: (.content.__typename // ""),
           status: (.fieldValueByName.name // "")
         }]')"

    local merged_json
    merged_json="$(jq -c -n --slurpfile a "$out" --argjson b "$chunk" '$a[0] + $b')"
    printf '%s\n' "$merged_json" > "$out"

    local has next
    has="$(printf '%s' "$page" | jq -r '.data.node.items.pageInfo.hasNextPage // false')"
    next="$(printf '%s' "$page" | jq -r '.data.node.items.pageInfo.endCursor // empty')"
    if [ "$has" != "true" ] || [ -z "$next" ] || [ "$collected" -ge "$LIMIT" ]; then
      break
    fi
    after="$next"
  done
  cat "$out"
}

# Apply Status→Done moves via aliased updateProjectV2ItemFieldValue (chunked).
apply_status_moves_chunked() {
  local done_id="$1"
  shift
  local -a move_ids=("$@")
  local n_moves="${#move_ids[@]}"
  [ "$n_moves" -gt 0 ] || return 0

  local chunk_size="${OVR_CHUNK:-40}" i=0
  while [ "$i" -lt "$n_moves" ]; do
    local mut_vars="\$projectId:ID!, \$fieldId:ID!, \$doneId:String!" body="" j=0
    local -a call=(gh api graphql)
    call+=(-f projectId="$PROJECT_ID" -f fieldId="$STATUS_FIELD_ID" -f doneId="$done_id")
    while [ "$j" -lt "$chunk_size" ] && [ $((i + j)) -lt "$n_moves" ]; do
      local idx=$((i + j))
      mut_vars+=", \$item$idx:ID!"
      body+=" m$idx: updateProjectV2ItemFieldValue(input: {"
      body+=" projectId: \$projectId, itemId: \$item$idx, fieldId: \$fieldId,"
      body+=" value: { singleSelectOptionId: \$doneId }"
      body+=" }) { projectV2Item { id } }"
      call+=(-f "item$idx=${move_ids[$idx]}")
      j=$((j + 1))
    done
    local mutation="mutation($mut_vars) {$body }"
    call+=(-f "query=$mutation")
    "${call[@]}" >/dev/null \
      || die "aliased Status update failed (chunk starting at index $i)"
    i=$((i + j))
  done
}

reconcile_backlog_help() {
  cat <<'EOF'
Usage:
  gh-board.sh reconcile-backlog [options]

Reconcile non-Done Status columns against local git evidence that work already
landed on the integration branch. Status field only — never force-closes issues.

Options:
  --owner O              Project owner override
  --repo O/R             Repository override
  --project-number N     Linked project number
  --base REF             Git ref to search (default: origin/dev; else default branch)
  --status NAME[,NAME]   Column(s) to scan (default: Backlog); mapped to closest options
  --apply                Apply moves (default: dry-run)
  --json                 Machine-readable JSON array
  --limit N              Max project items to list (default: 100)
  --chunk N              Aliased mutations per GraphQL HTTP call (default: 40)
  --help                 Show this help

Done heuristics (ANY strong signal → move to Done; else skip):
  - issue/PR state == CLOSED
  - git log <base> mentions #N / Closes #N / Fixes #N / Resolves #N / Refs #N
  - PR content with merged==true already in the list payload (no per-issue REST)

API shape (minimize HTTP calls):
  1× GraphQL projectV2.items list (paginated) + ceil(moves/chunk) aliased
  updateProjectV2ItemFieldValue mutations. Zero per-item gh project item-edit.
EOF
}

cmd_reconcile_backlog() {
  need gh; need jq
  if [ "${WANT_HELP:-0}" = "1" ]; then reconcile_backlog_help; return 0; fi

  resolve_status_board
  resolve_base_ref

  local done_line done_id done_name
  done_line="$(closest_status_option "Done" "$STATUS_OPTIONS")"
  [ -n "$done_line" ] || die "no Status option matching 'Done' on this board"
  done_id="${done_line%%$'\t'*}"; done_name="${done_line#*$'\t'}"

  # Resolve --status names (comma-separated) to actual option names on the board.
  local want_status="${OVR_STATUS:-Backlog}"
  local target_names=() part line oname
  while IFS= read -r part; do
    part="$(printf '%s' "$part" | sed 's/^[[:space:]]*//;s/[[:space:]]*$//')"
    [ -n "$part" ] || continue
    line="$(closest_status_option "$part" "$STATUS_OPTIONS")"
    [ -n "$line" ] || die "no Status option matching '$part' (available: $(
      printf '%s' "$STATUS_OPTIONS" | jq -r '.[].name' | paste -sd, -))"
    oname="${line#*$'\t'}"
    target_names+=("$oname")
  done < <(printf '%s\n' "$want_status" | tr ',' '\n')
  [ "${#target_names[@]}" -gt 0 ] || die "no --status columns resolved"

  err "listing project #$PROJECT_NUMBER items (limit=$LIMIT, base=$BASE_REF, status=${target_names[*]})"
  local items
  items="$(list_project_items_graphql)"

  local targets_json
  targets_json="$(printf '%s\n' "${target_names[@]}" | jq -R . | jq -s -c .)"

  # Filter to target columns, decide action per item (local git = 0 API).
  local decisions
  decisions="$(printf '%s' "$items" | jq -c --argjson targets "$targets_json" --arg done "$done_name" '
    [.[] | select(.status as $s | $targets | index($s) != null)
     | . + {from_status:.status, to_status:$done, action:"pending", reason:""}]')"

  local results_file; results_file="$(mktemp)"; _CLEANUP+=("$results_file")
  printf '[]' > "$results_file"

  local row number state merged title from_status item_id reason action
  while IFS= read -r row; do
    [ -z "$row" ] && continue
    number="$(printf '%s' "$row" | jq -r '.number')"
    state="$(printf '%s' "$row" | jq -r '.state')"
    merged="$(printf '%s' "$row" | jq -r '.merged')"
    title="$(printf '%s' "$row" | jq -r '.title')"
    from_status="$(printf '%s' "$row" | jq -r '.from_status')"
    item_id="$(printf '%s' "$row" | jq -r '.itemId')"
    reason=""; action="skip"

    if [ "$state" = "CLOSED" ]; then
      action="move"; reason="issue_closed"
    elif [ "$merged" = "true" ]; then
      action="move"; reason="pr_merged"
    elif git_mentions_issue "$number"; then
      action="move"; reason="commit_on_base"
    else
      reason="no_strong_signal"
    fi

    local applied="dry-run"
    [ "$action" = "move" ] || applied="skipped"
    [ "${OVR_APPLY:-0}" = "1" ] && [ "$action" = "move" ] && applied="pending"

    local entry
    entry="$(jq -nc \
      --argjson n "$number" \
      --arg title "$title" \
      --arg from "$from_status" \
      --arg to "$done_name" \
      --arg reason "$reason" \
      --arg action "$action" \
      --arg itemId "$item_id" \
      --arg applied "$applied" \
      '{number:$n, title:$title, from_status:$from, to_status:$to,
        reason:$reason, action:$action, itemId:$itemId, applied:$applied}')"
    jq -c -n --slurpfile a "$results_file" --argjson b "$entry" '$a[0] + [$b]' > "$results_file.tmp"
    mv "$results_file.tmp" "$results_file"
  done < <(printf '%s' "$decisions" | jq -c '.[]')

  # Apply moves in aliased GraphQL chunks (≤ ceil(N/chunk) HTTP calls).
  if [ "${OVR_APPLY:-0}" = "1" ]; then
    local move_ids=()
    while IFS= read -r item_id; do
      [ -n "$item_id" ] && move_ids+=("$item_id")
    done < <(jq -r '.[] | select(.action=="move") | .itemId' "$results_file")
    apply_status_moves_chunked "$done_id" "${move_ids[@]+"${move_ids[@]}"}"
    jq -c '[.[] | if .action=="move" then .applied="applied" else . end]' \
      "$results_file" > "$results_file.tmp"
    mv "$results_file.tmp" "$results_file"
  fi

  if [ "${OUT_JSON:-0}" = "1" ]; then
    jq '.' "$results_file"
  else
    local n_move n_skip mode
    n_move="$(jq '[.[] | select(.action=="move")] | length' "$results_file")"
    n_skip="$(jq '[.[] | select(.action=="skip")] | length' "$results_file")"
    mode="dry-run"; [ "${OVR_APPLY:-0}" = "1" ] && mode="apply"
    printf 'reconcile-backlog (%s) base=%s → Done=%s | move=%s skip=%s\n' \
      "$mode" "$BASE_REF" "$done_name" "$n_move" "$n_skip"
    if [ ! -s "$results_file" ] || [ "$(jq 'length' "$results_file")" = "0" ]; then
      printf 'no items in target Status column(s)\n'
    else
      printf '%-6s  %-8s  %-12s  %-12s  %-18s  %s  %s\n' \
        "NUMBER" "ACTION" "FROM" "TO" "REASON" "APPLIED" "TITLE"
      jq -r '.[] | [
          ("#" + (.number|tostring)),
          .action, .from_status, .to_status, .reason, .applied, .title
        ] | @tsv' "$results_file" \
        | while IFS=$'\t' read -r a b c d e f g; do
            printf '%-6s  %-8s  %-12s  %-12s  %-18s  %s  %s\n' "$a" "$b" "$c" "$d" "$e" "$f" "$g"
          done
    fi
  fi
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
  OVR_BASE=""; OVR_STATUS=""; OVR_APPLY=0; OVR_CHUNK=40
  OUT_JSON=0; LIMIT=100; WANT_HELP=0
  local positional=()
  while [ $# -gt 0 ]; do
    case "$1" in
      --owner) OVR_OWNER="$2"; shift 2;;
      --repo) OVR_REPO="$2"; shift 2;;
      --project-number) OVR_PNUM="$2"; shift 2;;
      --since) OVR_SINCE="$2"; shift 2;;
      --since-file) OVR_SINCEFILE="$2"; shift 2;;
      --interval) OVR_INTERVAL="$2"; shift 2;;
      --base) OVR_BASE="$2"; shift 2;;
      --status) OVR_STATUS="$2"; shift 2;;
      --chunk) OVR_CHUNK="$2"; shift 2;;
      --limit) LIMIT="$2"; shift 2;;
      --apply) OVR_APPLY=1; shift;;
      --json) OUT_JSON=1; shift;;
      --help|-h) WANT_HELP=1; shift;;
      *) positional+=("$1"); shift;;
    esac
  done
  case "$sub" in
    env) cmd_env;;
    item-id) cmd_item_id "${positional[0]:-}";;
    list-ready) cmd_list_ready;;
    events) cmd_events;;
    watch) cmd_watch;;
    reconcile-backlog) cmd_reconcile_backlog;;
    *) die "unknown subcommand: $sub (use: env | item-id | list-ready | events | watch | reconcile-backlog)";;
  esac
}

main "$@"
