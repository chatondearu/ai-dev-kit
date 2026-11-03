#!/usr/bin/env bash
# shellcheck shell=bash
# Live orchestration graph under $ORCH_REPO/.orch/graph/current.json (cdamux + foam-graph read this).

orch_graph_file() {
  printf '%s/.orch/graph/current.json\n' "${ORCH_REPO:-.}"
}

orch_graph_enabled() {
  case "${ORCH_GRAPH:-1}" in
    0|off|false|no) return 1;;
  esac
  return 0
}

orch_graph_now() {
  date -u +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || date -u +%Y-%m-%dT%H:%M:%SZ
}

orch_graph_init() {
  local run_id="${1:-run-$(date -u +%Y%m%dT%H%M%SZ)}"
  local gf dir
  gf="$(orch_graph_file)"
  dir="$(dirname "$gf")"
  mkdir -p "$dir"
  if ! command -v jq >/dev/null 2>&1; then
    printf '{"run_id":"%s","repo":"%s","nodes":[],"edges":[],"updated_at":"%s","status":"running"}\n' \
      "$run_id" "${ORCH_REPO:-.}" "$(orch_graph_now)" >"$gf"
    return 0
  fi
  jq -n \
    --arg run_id "$run_id" \
    --arg repo "${ORCH_REPO:-.}" \
    --arg updated "$(orch_graph_now)" \
    '{run_id:$run_id,repo:$repo,nodes:[],edges:[],updated_at:$updated,status:"running"}' >"$gf"
}

# Serialize jq read-modify-write (parallel graph-plan / multi-worker).
# Usage: orch_graph_locked_jq --arg k v 'filter'
orch_graph_locked_jq() {
  local gf dir lock tmp rc=0
  gf="$(orch_graph_file)"
  dir="$(dirname "$gf")"
  mkdir -p "$dir"
  lock="$dir/.lock"
  tmp="$(mktemp)"
  if command -v flock >/dev/null 2>&1; then
    (
      flock 200
      if ! jq "$@" "$gf" >"$tmp"; then
        rm -f "$tmp"
        exit 1
      fi
      mv "$tmp" "$gf"
    ) 200>"$lock" || rc=$?
  else
    if ! jq "$@" "$gf" >"$tmp"; then
      rm -f "$tmp"
      return 1
    fi
    mv "$tmp" "$gf" || rc=$?
  fi
  return "$rc"
}

# Mark run finished (ok|blocked). Sets .status and .ended_at for cdamux freshness.
orch_graph_finish() {
  local outcome="${1:-ok}"
  local gf status
  if ! orch_graph_enabled; then
    return 0
  fi
  gf="$(orch_graph_file)"
  [ -f "$gf" ] || return 0
  case "$outcome" in
    ok|done|success) status="ok";;
    *) status="blocked";;
  esac
  if ! command -v jq >/dev/null 2>&1; then
    return 0
  fi
  orch_graph_locked_jq \
    --arg status "$status" \
    --arg ended "$(orch_graph_now)" \
    --arg updated "$(orch_graph_now)" \
    '.status = $status | .ended_at = $ended | .updated_at = $updated'

  # Archive snapshot for cdamux recent-runs (keep last 20).
  local runs_dir run_id dest safe
  runs_dir="$(dirname "$gf")/runs"
  mkdir -p "$runs_dir"
  run_id="$(jq -r '.run_id // empty' "$gf" 2>/dev/null || true)"
  [ -n "$run_id" ] || return 0
  safe="$(printf '%s' "$run_id" | tr -c 'A-Za-z0-9._-' '_')"
  dest="$runs_dir/${safe}.json"
  cp "$gf" "$dest" 2>/dev/null || true
  # shellcheck disable=SC2012
  ls -1t "$runs_dir"/*.json 2>/dev/null | tail -n +21 | while read -r old; do
    rm -f "$old"
  done
}

# Warn when worker-result.json is missing or lacks required fields (issue/status/checks).
orch_worker_result_warn() {
  local wt="$1"
  local f="$wt/.orch/worker-result.json"
  if [ ! -f "$f" ]; then
    printf 'orch: warn: missing %s (integrator/graph fan-in weaker)\n' "$f" >&2
    return 0
  fi
  if ! command -v jq >/dev/null 2>&1; then
    return 0
  fi
  if ! jq -e '(.issue | type == "number") and (.status == "done" or .status == "blocked") and (.checks == "green" or .checks == "red" or .checks == "none")' "$f" >/dev/null 2>&1; then
    printf 'orch: warn: %s missing required fields (issue, status, checks)\n' "$f" >&2
  fi
}

orch_graph_touch() {
  local gf
  gf="$(orch_graph_file)"
  [ -f "$gf" ] || orch_graph_init
  if ! command -v jq >/dev/null 2>&1; then
    return 0
  fi
  orch_graph_locked_jq --arg updated "$(orch_graph_now)" '.updated_at = $updated'
}

# orch_graph_upsert_node ID kind phase status message [parent]
orch_graph_upsert_node() {
  local id="$1" kind="$2" phase="$3" status="$4" msg="$5"
  local parent="${6:-}"
  local gf
  if ! orch_graph_enabled; then
    return 0
  fi
  gf="$(orch_graph_file)"
  [ -f "$gf" ] || orch_graph_init
  if ! command -v jq >/dev/null 2>&1; then
    return 0
  fi
  orch_graph_locked_jq \
    --arg id "$id" \
    --arg kind "$kind" \
    --arg phase "$phase" \
    --arg status "$status" \
    --arg msg "$msg" \
    --arg parent "$parent" \
    --arg updated "$(orch_graph_now)" \
    '
      .updated_at = $updated
      | .nodes = (
          [.nodes[] | select(.id != $id)]
          + [{
              id: $id,
              kind: $kind,
              phase: $phase,
              status: $status,
              message: $msg,
              parent: (if $parent == "" then null else $parent end)
            }]
        )
    '
}

# orch_graph_add_edge FROM TO TYPE
orch_graph_add_edge() {
  local from="$1" to="$2" etype="${3:-depends}"
  local gf
  if ! orch_graph_enabled || ! command -v jq >/dev/null 2>&1; then
    return 0
  fi
  gf="$(orch_graph_file)"
  [ -f "$gf" ] || return 0
  orch_graph_locked_jq \
    --arg from "$from" \
    --arg to "$to" \
    --arg type "$etype" \
    --arg updated "$(orch_graph_now)" \
    '
      .updated_at = $updated
      | .edges = (
          [.edges[] | select(.from != $from or .to != $to or .type != $type)]
          + [{from:$from,to:$to,type:$type}]
        )
    '
}

# The worker loop (orch_loop_worker delegates here): implement/fix nodes + verify +
# grounding verifier. Graph writes are no-ops when ORCH_GRAPH=0 (--no-graph).
orch_graph_worker() {
  local wt="$1" prompt="$2"
  local max="${ORCH_MAX_ITER:-10}" i=1
  local agent_rc verify_rc vcmd wid="${ORCH_WORKER_ID:-worker}"
  local node_prefix="worker-${wid}"
  local base_prompt="$prompt" vlog="$wt/.orch/verify.log"

  orch_graph_upsert_node "$node_prefix" worker pending queued "worker $wid" ""
  orch_graph_add_edge "integrator" "$node_prefix" aggregates

  while [ "$i" -le "$max" ]; do
    local phase="implement"
    [ "$i" -gt 1 ] && phase="fix"
    local agent_node="${node_prefix}-${phase}-${i}"

    orch_graph_upsert_node "$agent_node" agent "$phase" running "iter ${i}/${max}" "$node_prefix"
    orch_graph_add_edge "$node_prefix" "$agent_node" step

    if type orch_progress_set >/dev/null 2>&1 && [ -n "${ORCH_WORKER_ID:-}" ]; then
      orch_progress_set "$ORCH_WORKER_ID" agent "graph ${phase} ${i}/${max}"
    else
      printf 'orch loop iter %s/%s (%s)\n' "$i" "$max" "$phase" >&2
    fi

    rm -f "$wt/.orch/agent-ok" 2>/dev/null || true
    agent_rc=0
    orch_agent_run "$wt" "$prompt" || agent_rc=$?

    orch_graph_upsert_node "$agent_node" agent "$phase" done "agent exit $agent_rc" "$node_prefix"

    local verify_node="${node_prefix}-verify-${i}"
    orch_graph_upsert_node "$verify_node" verify verify running "running verify" "$node_prefix"
    orch_graph_add_edge "$agent_node" "$verify_node" step

    if ! vcmd="$(orch_resolve_verify "$wt")"; then
      orch_graph_upsert_node "$verify_node" verify verify failed "no verify command" "$node_prefix"
      orch_graph_upsert_node "$node_prefix" worker blocked blocked "$ORCH_NO_VERIFY_REASON" ""
      if type orch_progress_set >/dev/null 2>&1 && [ -n "${ORCH_WORKER_ID:-}" ]; then
        orch_progress_set "$ORCH_WORKER_ID" blocked "$ORCH_NO_VERIFY_REASON"
      else
        printf 'blocked: %s\n' "$ORCH_NO_VERIFY_REASON" >&2
      fi
      return 3
    fi
    if type orch_progress_set >/dev/null 2>&1 && [ -n "${ORCH_WORKER_ID:-}" ]; then
      orch_progress_set "$ORCH_WORKER_ID" verify "${vcmd%%$'\n'*}"
    fi

    mkdir -p "$wt/.orch"
    orch_run_verify "$wt" 2>&1 | tee "$vlog" >&2
    verify_rc="${PIPESTATUS[0]}"
    if [ "$verify_rc" -eq 0 ]; then
      orch_graph_upsert_node "$verify_node" verify verify done "verify ok" "$node_prefix"

      if [ "$vcmd" = "true" ] && [ "$agent_rc" -ne 0 ] && [ ! -f "$wt/.orch/agent-ok" ]; then
        orch_graph_upsert_node "$node_prefix" worker "$phase" retry "agent failed, verify fallback" ""
        if [ -n "${ORCH_WORKER_ID:-}" ] && type orch_progress_set >/dev/null 2>&1; then
          orch_progress_set "$ORCH_WORKER_ID" agent "retry after agent fail (${i}/${max})"
        else
          printf 'orch loop: agent failed and verify is fallback true (iter %s/%s)\n' "$i" "$max" >&2
        fi
        i=$((i + 1))
        continue
      fi

      # Default on (ADR-006): the grounding verifier is part of the gate.
      case "${ORCH_GRAPH_VERIFY:-1}" in
        1|on|true|yes)
          if orch_graph_verifier_pass "$wt" "$node_prefix"; then
            orch_graph_upsert_node "$node_prefix" worker done done "verify + grounding ok" ""
            [ -n "${ORCH_WORKER_ID:-}" ] || printf 'verify ok\n' >&2
            return 0
          fi
          orch_graph_upsert_node "$node_prefix" worker verify blocked "grounding verifier failed" ""
          [ -n "${ORCH_WORKER_ID:-}" ] || printf 'orch loop: grounding verifier failed (iter %s/%s)\n' "$i" "$max" >&2
          ;;
        *)
          orch_graph_upsert_node "$node_prefix" worker done done "verify ok" ""
          [ -n "${ORCH_WORKER_ID:-}" ] || printf 'verify ok\n' >&2
          return 0
          ;;
      esac
    else
      orch_graph_upsert_node "$verify_node" verify verify failed "verify failed" "$node_prefix"
      orch_graph_upsert_node "$node_prefix" worker verify retry "verify failed iter $i" ""
      # Retry with the failing output instead of re-sending the same prompt blind.
      if type orch_verify_retry_prompt >/dev/null 2>&1; then
        prompt="$(orch_verify_retry_prompt "$base_prompt" "$wt" "$vlog")"
      fi
    fi
    i=$((i + 1))
  done

  orch_graph_upsert_node "$node_prefix" worker blocked blocked "max iter exhausted" ""
  [ -n "${ORCH_WORKER_ID:-}" ] || printf 'blocked\n' >&2
  return 1
}

orch_graph_verifier_pass() {
  local wt="$1" parent="$2"
  local prompts="${ORCH_PROMPTS_DIR:-}"
  local tpl="$prompts/verifier.md"
  local vprompt="$wt/.orch/verifier-prompt.md"
  local plan="none"

  local p repo="${ORCH_REPO:-$wt}" wid="${ORCH_WORKER_ID:-0}"
  for p in \
    "$repo/plans/PLAN-${wid}"-*.md \
    "$wt/plans/PLAN-${wid}"-*.md
  do
    [ -f "$p" ] || continue
    plan="$p"
    break
  done
  # Fast lane: no plan, the agent brief is the contract.
  [ "$plan" = none ] && [ -f "$wt/.orch/brief.md" ] && plan="$wt/.orch/brief.md"

  if [ ! -f "$tpl" ]; then
    return 0
  fi
  orch_resolve_context "$repo" "$wid"
  orch_render_prompt "$tpl" "$vprompt" \
    "WORKTREE=$wt" \
    "ISSUE=${ORCH_WORKER_ID:-N/A}" \
    "PLAN_PATH=$plan" \
    "CONSTRAINTS=$ORCH_CONSTRAINTS"

  local vnode="${parent}-verifier"
  orch_graph_upsert_node "$vnode" verifier grounding running "checking plan/ADR" "$parent"
  orch_graph_add_edge "$parent" "$vnode" step

  # Markers from a previous pass (review fix, reused worktree) must not decide this one.
  rm -f "$wt/.orch/agent-ok" "$wt/.orch/verifier-ok" "$wt/.orch/verifier-fail" 2>/dev/null || true
  if orch_agent_run "$wt" "$vprompt"; then
    if [ -f "$wt/.orch/verifier-ok" ]; then
      orch_graph_upsert_node "$vnode" verifier grounding done "grounded" "$parent"
      return 0
    fi
  fi
  orch_graph_upsert_node "$vnode" verifier grounding failed "not grounded" "$parent"
  return 1
}

# Run integrator agent at repo root after parallel workers (OpenClaw-style synthesize).
orch_graph_integrator_run() {
  local ids=("$@")
  local gf prompts="$ORCH_PROMPTS_DIR"
  local tpl="$prompts/integrator.md"
  local out="$ORCH_REPO/.orch/integrator-prompt.md"
  local summaries="" id path wt

  [ "${#ids[@]}" -ge 1 ] || return 0
  case "${ORCH_GRAPH_INTEGRATOR:-auto}" in
    0|off|false|no)
      orch_graph_upsert_node integrator integrator skipped skipped "ORCH_GRAPH_INTEGRATOR off" ""
      return 0
      ;;
    auto)
      if [ "${#ids[@]}" -lt 2 ]; then
        orch_graph_upsert_node integrator integrator skipped skipped "single issue — no synthesize" ""
        return 0
      fi
      ;;
  esac

  orch_graph_upsert_node integrator integrator synthesize running "collecting worker outputs" ""
  for id in "${ids[@]}"; do
    path="$(orch_worktree_path "$ORCH_REPO" "$id" "issue-$id")"
    wt="$path"
    if [ -f "$wt/.orch/worker-result.json" ]; then
      summaries="${summaries}
--- worker #${id} ---
$(cat "$wt/.orch/worker-result.json")
"
    elif [ -f "$ORCH_REPO/.orch/run/${id}.status" ]; then
      summaries="${summaries}
--- worker #${id} ---
$(cat "$ORCH_REPO/.orch/run/${id}.status")
"
    fi
    orch_graph_add_edge integrator "worker-${id}" aggregates
  done

  if [ ! -f "$tpl" ]; then
    orch_graph_upsert_node integrator integrator synthesize done "no integrator template" ""
    return 0
  fi

  orch_render_prompt "$tpl" "$out" \
    "REPO_ROOT=$ORCH_REPO" \
    "WORKER_SUMMARIES=see below"

  {
    cat "$out"
    printf '\n\n## Worker payloads\n%s\n' "$summaries"
  } >"$out"

  export ORCH_WT="$ORCH_REPO"
  export ORCH_WORKER_ID=""
  if orch_agent_run "$ORCH_REPO" "$out"; then
    orch_graph_upsert_node integrator integrator synthesize done "synthesis complete" ""
    return 0
  fi
  orch_graph_upsert_node integrator integrator synthesize failed "integrator agent failed" ""
  return 1
}

# Normalize path prefix for overlap checks (trailing slash, no leading ./).
orch_graph_norm_path() {
  local p="${1%/}"
  p="${p#./}"
  printf '%s/\n' "$p"
}

# Return 0 when two normalized path prefixes overlap.
orch_graph_paths_overlap() {
  local a b
  a="$(orch_graph_norm_path "$1")"
  b="$(orch_graph_norm_path "$2")"
  case "$a" in "$b"*) return 0;; esac
  case "$b" in "$a"*) return 0;; esac
  return 1
}

# Exit 1 if any task path overlaps another (cross-task).
orch_graph_plan_validate_disjoint() {
  local plan_file="$1"
  local n i j pi pj paths_i paths_j
  n="$(jq '.tasks | length' "$plan_file")"
  i=0
  while [ "$i" -lt "$n" ]; do
    j=$((i + 1))
    while [ "$j" -lt "$n" ]; do
      while IFS= read -r pi; do
        [ -n "$pi" ] || continue
        while IFS= read -r pj; do
          [ -n "$pj" ] || continue
          if orch_graph_paths_overlap "$pi" "$pj"; then
            printf 'graph-plan: path overlap task %s vs %s: %s / %s\n' \
              "$(jq -r ".tasks[$i].id" "$plan_file")" \
              "$(jq -r ".tasks[$j].id" "$plan_file")" "$pi" "$pj" >&2
            return 1
          fi
        done < <(jq -r ".tasks[$j].paths[]?" "$plan_file")
      done < <(jq -r ".tasks[$i].paths[]?" "$plan_file")
      j=$((j + 1))
    done
    i=$((i + 1))
  done
  return 0
}

# 1 = run tasks in parallel when paths are disjoint; 0 = always sequential.
orch_graph_plan_wants_parallel() {
  case "${ORCH_GRAPH_PLAN_PARALLEL:-auto}" in
    0|off|false|no|sequential) return 1;;
    1|on|true|yes|parallel) return 0;;
    auto)
      orch_graph_plan_validate_disjoint "$1"
      ;;
    *) return 1;;
  esac
}

# Files changed in WT since BASE (commits + tracked/untracked edits), minus .orch/.
orch_graph_changed_files() {
  local wt="$1" base="$2"
  {
    git -C "$wt" diff --name-only "$base" HEAD 2>/dev/null
    git -C "$wt" status --porcelain -uall 2>/dev/null | cut -c4- | sed 's/.* -> //'
  } | { grep -v '^\.orch/' || true; } | sort -u
}

# Return 0 when FILE matches one of the path prefixes/globs given after it.
orch_graph_path_allowed() {
  local f="$1" p
  shift
  for p in "$@"; do
    p="${p%/}"
    # shellcheck disable=SC2254
    case "$f" in $p|$p/*) return 0;; esac
  done
  return 1
}

# Print files changed since BASE that fall outside task I's `paths` (empty = ok).
# BASELINE = files already dirty before the task. In parallel mode the worktree
# is shared, so other tasks' paths are tolerated (the writer cannot be told apart).
orch_graph_paths_violations() {
  local plan_file="$1" i="$2" wt="$3" base="$4" baseline="$5" parallel="${6:-0}"
  local own=() other=() f
  mapfile -t own < <(jq -r ".tasks[$i].paths[]?" "$plan_file")
  [ "${#own[@]}" -gt 0 ] || return 0
  if [ "$parallel" = 1 ]; then
    mapfile -t other < <(jq -r --argjson i "$i" '.tasks | to_entries[] | select(.key != $i) | .value.paths[]?' "$plan_file")
  fi
  while IFS= read -r f; do
    [ -n "$f" ] || continue
    grep -qxF -- "$f" <<<"$baseline" && continue
    orch_graph_path_allowed "$f" "${own[@]}" && continue
    if [ "${#other[@]}" -gt 0 ] && orch_graph_path_allowed "$f" "${other[@]}"; then
      continue
    fi
    printf '%s\n' "$f"
  done < <(orch_graph_changed_files "$wt" "$base")
}

orch_graph_plan_run_task() {
  local plan_file="$1" path="$2" issue="$3" i="$4" parallel="${5:-0}"
  local tid body paths prompt_file rc_task base baseline bad
  tid="$(jq -r ".tasks[$i].id" "$plan_file")"
  body="$(jq -r ".tasks[$i].prompt" "$plan_file")"
  paths="$(jq -r ".tasks[$i].paths | join(\", \")" "$plan_file")"
  [ -n "$tid" ] && [ "$tid" != "null" ] || tid="task-$i"

  orch_graph_upsert_node "task-${tid}" worker task pending "queued" ""
  orch_graph_add_edge integrator "task-${tid}" aggregates

  prompt_file="$path/.orch/task-${tid}-prompt.md"
  {
    printf '# Graph plan task %s (issue #%s)\n\n' "$tid" "$issue"
    printf 'Worktree: %s\nOnly modify files under: %s\n\n' "$path" "$paths"
    printf '%s\n\n' "$body"
    printf 'Write `.orch/task-%s-result.json` when done.\n' "$tid"
  } >"$prompt_file"

  rc_task="$path/.orch/task-${tid}.rc"
  base="$(git -C "$path" rev-parse HEAD)"
  baseline="$(orch_graph_changed_files "$path" "$base")"
  export ORCH_WT="$path"
  export ORCH_WORKER_ID="task-${tid}"
  orch_progress_set "$issue" agent "task ${tid}"
  if ! orch_sandbox_wrap bash -c "ORCH_WT=\"$path\" ORCH_WORKER_ID=\"task-${tid}\" ORCH_REPO=\"$ORCH_REPO\" orch_graph_worker \"$path\" \"$prompt_file\""; then
    orch_graph_upsert_node "task-${tid}" worker task failed "agent failed" ""
    printf '1' >"$rc_task"
    return 1
  fi
  bad="$(orch_graph_paths_violations "$plan_file" "$i" "$path" "$base" "$baseline" "$parallel" | paste -sd, -)"
  if [ -n "$bad" ]; then
    orch_graph_upsert_node "task-${tid}" worker task blocked "paths: $bad" ""
    printf 'graph-plan: task %s blocked, files outside paths: %s\n' "$tid" "$bad" >&2
    printf '1' >"$rc_task"
    return 1
  fi
  orch_graph_upsert_node "task-${tid}" worker task done "complete" ""
  printf '0' >"$rc_task"
  return 0
}

# Intra-issue fan-out from JSON plan (opt-in --graph-plan).
orch_graph_plan_run() {
  local plan_file="$1" issue="$2"
  # Split locals: bash expands ${slug} before assignment in the same `local` line.
  local slug="issue-$issue"
  local branch="feat/${issue}-${slug}"
  local path n i rc_dir rc_file parallel=0 max_p

  command -v jq >/dev/null 2>&1 || {
    printf 'graph-plan: jq required\n' >&2
    return 1
  }
  [ -f "$plan_file" ] || {
    printf 'graph-plan: missing file %s\n' "$plan_file" >&2
    return 1
  }

  local plan_issue
  plan_issue="$(jq -r '.issue // empty' "$plan_file")"
  if [ -n "$plan_issue" ] && [ "$plan_issue" != "$issue" ]; then
    printf 'graph-plan: plan issue %s != --issue %s\n' "$plan_issue" "$issue" >&2
    return 1
  fi

  n="$(jq '.tasks | length' "$plan_file")"
  if [ -z "$n" ] || [ "$n" = "0" ] || [ "$n" = "null" ]; then
    printf 'graph-plan: tasks array empty\n' >&2
    return 1
  fi

  if orch_graph_plan_wants_parallel "$plan_file"; then
    parallel=1
    if ! orch_graph_plan_validate_disjoint "$plan_file"; then
      printf 'graph-plan: ORCH_GRAPH_PLAN_PARALLEL=parallel but paths overlap\n' >&2
      return 1
    fi
    printf 'graph-plan: parallel dispatch (%s tasks, disjoint paths)\n' "$n" >&2
  else
    printf 'graph-plan: sequential dispatch (%s tasks)\n' "$n" >&2
  fi

  orch_graph_init "graph-plan-${issue}-$(date -u +%Y%m%dT%H%M%SZ)"
  orch_graph_upsert_node integrator integrator pending queued "graph plan tasks" ""

  path="$(orch_worktree_path "$ORCH_REPO" "$issue" "$slug")"
  rc_dir="$ORCH_REPO/.orch/run"
  mkdir -p "$rc_dir"
  rc_file="$rc_dir/${issue}.rc"

  export ORCH_WORKER_ID="$issue"
  orch_progress_set "$issue" worktree "graph-plan ${path##*/}"
  if ! orch_worktree_add "$ORCH_REPO" "$issue" "$slug" "$branch" HEAD; then
    orch_progress_set "$issue" blocked "worktree create failed"
    echo 1 >"$rc_file"
    return 1
  fi
  orch_worktree_bootstrap "$ORCH_REPO" "$path"
  mkdir -p "$path/.orch"

  if [ "$parallel" = 0 ]; then
    i=0
    while [ "$i" -lt "$n" ]; do
      if ! orch_graph_plan_run_task "$plan_file" "$path" "$issue" "$i"; then
        orch_progress_set "$issue" blocked "graph-plan task failed"
        echo 1 >"$rc_file"
        return 1
      fi
      i=$((i + 1))
    done
  else
    max_p="${ORCH_GRAPH_PLAN_MAX_PARALLEL:-${ORCH_MAX_PARALLEL:-3}}"
    i=0
    while [ "$i" -lt "$n" ]; do
      orch_pool_slot "$max_p"
      (
        set +e
        orch_graph_plan_run_task "$plan_file" "$path" "$issue" "$i" 1
        exit $?
      ) &
      orch_pool_add $!
      i=$((i + 1))
    done
    orch_pool_drain
    i=0
    while [ "$i" -lt "$n" ]; do
      tid="$(jq -r ".tasks[$i].id" "$plan_file")"
      [ -n "$tid" ] && [ "$tid" != "null" ] || tid="task-$i"
      if [ -f "$path/.orch/task-${tid}.rc" ] && [ "$(cat "$path/.orch/task-${tid}.rc")" != "0" ]; then
        orch_progress_set "$issue" blocked "graph-plan parallel task ${tid} failed"
        echo 1 >"$rc_file"
        return 1
      fi
      i=$((i + 1))
    done
  fi

  export ORCH_GRAPH_INTEGRATOR=1
  orch_graph_integrator_plan_run "$path" "$plan_file" || true

  touch "$path/.orch/graph-plan-complete"
  orch_progress_set "$issue" agent "graph plan done — PR/CI"
  return 0
}

orch_graph_integrator_plan_run() {
  local wt="$1" plan_file="$2"
  local summaries="" n i tid prompts="$ORCH_PROMPTS_DIR" tpl out
  tpl="$prompts/integrator.md"
  out="$ORCH_REPO/.orch/integrator-plan-prompt.md"
  n="$(jq '.tasks | length' "$plan_file")"
  i=0
  while [ "$i" -lt "$n" ]; do
    tid="$(jq -r ".tasks[$i].id" "$plan_file")"
    if [ -f "$wt/.orch/task-${tid}-result.json" ]; then
      summaries="${summaries}
--- task ${tid} ---
$(cat "$wt/.orch/task-${tid}-result.json")
"
    elif [ -f "$wt/.orch/worker-result.json" ]; then
      summaries="${summaries}
--- task ${tid} ---
$(cat "$wt/.orch/worker-result.json")
"
    fi
    orch_graph_add_edge integrator "task-${tid}" aggregates
    i=$((i + 1))
  done
  [ -f "$tpl" ] || return 0
  orch_render_prompt "$tpl" "$out" "REPO_ROOT=$ORCH_REPO" "WORKER_SUMMARIES=see below"
  { cat "$out"; printf '\n\n## Task payloads\n%s\n' "$summaries"; } >"$out"
  export ORCH_WT="$ORCH_REPO"
  orch_agent_run "$ORCH_REPO" "$out" || return 0
  orch_graph_upsert_node integrator integrator synthesize done "plan synthesis" ""
}
