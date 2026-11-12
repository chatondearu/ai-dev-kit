#!/usr/bin/env bash
# shellcheck shell=bash
# Live orchestration graph under $CDAMUX_HOME/runs/<run_id>.json via `cdamux state`
# (ADR-011 §5 hard cut #252). Without cdamux: silent no-op (ADR-001). Repo `.orch/`
# keeps work artefacts only — no status.json / .orch/graph writers.

_ORCH_CDAMUX_STATE_LIB="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/cdamux_state.sh"
# shellcheck source=cdamux_state.sh
source "$_ORCH_CDAMUX_STATE_LIB"
unset _ORCH_CDAMUX_STATE_LIB

# Path of the run file under CDAMUX_HOME when resolvable (readers/tests); else empty.
orch_graph_file() {
  orch_cdamux_run_path "${ORCH_RUN_ID:-}" 2>/dev/null || true
}

orch_graph_enabled() {
  case "${ORCH_GRAPH:-1}" in
    0|off|false|no) return 1;;
  esac
  return 0
}

# Graph state needs cdamux on PATH (hard cut — no in-repo fallback).
orch_graph_writable() {
  orch_graph_enabled && orch_cdamux_available && command -v jq >/dev/null 2>&1
}

orch_graph_now() {
  date -u +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || date -u +%Y-%m-%dT%H:%M:%SZ
}

# tmux session name of the caller when inside tmux; empty (fail-open) otherwise.
orch_graph_session() {
  [ -n "${TMUX:-}" ] || return 0
  tmux display-message -p '#S' 2>/dev/null || true
}

# No-ops kept so `export -f` lists from older callers/tests stay valid.
orch_graph_point_current() { :; }
orch_graph_archive() { :; }
orch_graph_archive_stale() { :; }

# Compat name: publish is now the write itself (via locked jq → state set).
orch_cdamux_publish_run() {
  local gf json
  orch_graph_writable || return 0
  [ -n "${ORCH_RUN_ID:-}" ] || return 0
  json="$(orch_cdamux_state_get runs "$ORCH_RUN_ID" 2>/dev/null || true)"
  [ -n "$json" ] || return 0
  orch_cdamux_publish_run_json "$json"
}

orch_graph_init() {
  export ORCH_RUN_ID="${1:-${ORCH_RUN_ID:-run-$(date -u +%Y%m%dT%H%M%SZ)}}"
  local session json repo
  orch_graph_writable || return 0
  session="$(orch_graph_session)"
  repo="$(cd "${ORCH_REPO:-.}" 2>/dev/null && pwd || printf '%s' "${ORCH_REPO:-.}")"
  json="$(jq -n \
    --arg run_id "$ORCH_RUN_ID" \
    --arg repo "$repo" \
    --arg session "$session" \
    --argjson pid "$$" \
    --arg updated "$(orch_graph_now)" \
    '{run_id:$run_id,repo:$repo,pid:$pid,nodes:[],edges:[],updated_at:$updated,status:"running"}
     + (if $session == "" then {} else {session:$session} end)')"
  orch_cdamux_publish_run_json "$json"
}

# Serialize jq read-modify-write via cdamux state get/set + repo lock artefact.
# Usage: orch_graph_locked_jq --arg k v 'filter'
orch_graph_locked_jq() {
  local lock tmp rc=0 raw out
  orch_graph_writable || return 0
  [ -n "${ORCH_RUN_ID:-}" ] || return 1
  lock="$(orch_cdamux_run_lock "$ORCH_RUN_ID")" || return 1
  tmp="$(mktemp "${TMPDIR:-/tmp}/orch-graph.XXXXXX")"
  # Shared body for flock/no-flock paths (args forwarded to jq).
  # Load existing run JSON; never invent a skeleton when the file already exists
  # (a failed `state get` must not wipe nodes via publish).
  _orch_graph_rmw_load() {
    raw="$(orch_cdamux_state_get runs "$ORCH_RUN_ID" 2>/dev/null || true)"
    if [ -n "$raw" ]; then
      return 0
    fi
    local f
    f="$(orch_cdamux_run_path "$ORCH_RUN_ID" 2>/dev/null || true)"
    if [ -n "$f" ] && [ -f "$f" ]; then
      raw="$(cat "$f" 2>/dev/null || true)"
      [ -n "$raw" ] || return 1
      return 0
    fi
    raw="$(jq -n --arg id "$ORCH_RUN_ID" '{run_id:$id,nodes:[],edges:[],status:"running"}')"
  }
  _do() {
    _orch_graph_rmw_load || return 1
    if ! printf '%s' "$raw" | jq "$@" >"$tmp"; then
      rm -f "$tmp"
      return 1
    fi
    out="$(cat "$tmp")"
    rm -f "$tmp"
    orch_cdamux_publish_run_json "$out"
  }
  if command -v flock >/dev/null 2>&1; then
    export -f orch_cdamux_available orch_cdamux_state_get orch_cdamux_state_set
    export -f orch_cdamux_publish_run_json orch_cdamux_home orch_cdamux_run_path
    export -f _orch_graph_rmw_load
    (
      flock 200
      _orch_graph_rmw_load || exit 1
      if ! printf '%s' "$raw" | jq "$@" >"$tmp"; then
        rm -f "$tmp"
        exit 1
      fi
      out="$(cat "$tmp")"
      rm -f "$tmp"
      orch_cdamux_publish_run_json "$out"
    ) 200>"$lock" || rc=$?
  else
    _do "$@" || rc=$?
  fi
  return "$rc"
}

# Mark run finished (ok|blocked); status lives under CDAMUX_HOME (no archive move).
orch_graph_finish() {
  local outcome="${1:-ok}" status
  if ! orch_graph_writable; then
    return 0
  fi
  [ -n "${ORCH_RUN_ID:-}" ] || return 0
  case "$outcome" in
    ok|done|success) status="ok";;
    *) status="blocked";;
  esac
  orch_graph_locked_jq \
    --arg status "$status" \
    --arg ended "$(orch_graph_now)" \
    --arg updated "$(orch_graph_now)" \
    '.status = $status | .ended_at = $ended | .updated_at = $updated'
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

# orch_graph_upsert_node ID kind phase status message [parent] [agent]
# Non-empty `agent`: attach cli/model/effort/tier of the last orch_agent_run call.
orch_graph_upsert_node() {
  local id="$1" kind="$2" phase="$3" status="$4" msg="$5"
  local parent="${6:-}" agent="${7:-}"
  if ! orch_graph_writable; then
    return 0
  fi
  [ -n "${ORCH_RUN_ID:-}" ] || orch_graph_init
  # Init only when the run file is absent. A failed `state get` must not call
  # orch_graph_init (that republishes an empty skeleton and wipes other nodes).
  local run_file
  run_file="$(orch_cdamux_run_path "$ORCH_RUN_ID" 2>/dev/null || true)"
  if [ -z "$run_file" ] || [ ! -f "$run_file" ]; then
    orch_graph_init "$ORCH_RUN_ID"
  fi
  orch_graph_locked_jq \
    --arg id "$id" \
    --arg kind "$kind" \
    --arg phase "$phase" \
    --arg status "$status" \
    --arg msg "$msg" \
    --arg parent "$parent" \
    --arg agent "$agent" \
    --arg cli "${ORCH_AGENT_CLI:-}" \
    --arg model "${ORCH_AGENT_MODEL:-}" \
    --arg effort "${ORCH_AGENT_EFFORT:-}" \
    --arg tier "${ORCH_AGENT_TIER:-}" \
    --arg updated "$(orch_graph_now)" \
    '
      .updated_at = $updated
      | .nodes = (
          [(.nodes // [])[] | select(.id != $id)]
          + [{
              id: $id,
              kind: $kind,
              phase: $phase,
              status: $status,
              message: $msg,
              parent: (if $parent == "" then null else $parent end)
            }
            + (if $agent == "" then {}
               else {cli:$cli, model:$model, effort:$effort, tier:$tier}
                 | map_values(if . == "" then null else . end) end)]
        )
    '
}

# orch_graph_add_edge FROM TO TYPE
orch_graph_add_edge() {
  local from="$1" to="$2" etype="${3:-depends}"
  if ! orch_graph_writable || [ -z "${ORCH_RUN_ID:-}" ]; then
    return 0
  fi
  if ! orch_cdamux_state_get runs "$ORCH_RUN_ID" >/dev/null 2>&1; then
    return 0
  fi
  orch_graph_locked_jq \
    --arg from "$from" \
    --arg to "$to" \
    --arg type "$etype" \
    --arg updated "$(orch_graph_now)" \
    '
      .updated_at = $updated
      | .edges = (
          [(.edges // [])[] | select(.from != $from or .to != $to or .type != $type)]
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
  local role="${ORCH_ROLE:-worker}" fail esc_rc reason
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
    # pr-fix runs this same loop with ORCH_ROLE=pr-fix (orch.sh).
    ORCH_ROLE="$role" orch_agent_run "$wt" "$prompt" || agent_rc=$?

    orch_graph_upsert_node "$agent_node" agent "$phase" done "agent exit $agent_rc" "$node_prefix" agent

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
          fail="grounding verifier failed"
          orch_graph_upsert_node "$node_prefix" worker verify blocked "$fail" ""
          # Retry with the verifier's reasons instead of re-sending the same prompt blind.
          if type orch_verifier_retry_prompt >/dev/null 2>&1; then
            prompt="$(orch_verifier_retry_prompt "$base_prompt" "$wt")"
          fi
          [ -n "${ORCH_WORKER_ID:-}" ] || printf 'orch loop: grounding verifier failed (iter %s/%s)\n' "$i" "$max" >&2
          ;;
        *)
          orch_graph_upsert_node "$node_prefix" worker done done "verify ok" ""
          [ -n "${ORCH_WORKER_ID:-}" ] || printf 'verify ok\n' >&2
          return 0
          ;;
      esac
    else
      fail="verify failed"
      orch_graph_upsert_node "$verify_node" verify verify failed "$fail" "$node_prefix"
      orch_graph_upsert_node "$node_prefix" worker verify retry "verify failed iter $i" ""
      # Retry with the failing output instead of re-sending the same prompt blind.
      if type orch_verify_retry_prompt >/dev/null 2>&1; then
        prompt="$(orch_verify_retry_prompt "$base_prompt" "$wt" "$vlog")"
      fi
    fi
    # Red verify or failed verifier: the next iteration runs one tier up, bounded by
    # ORCH_TIER_MAX (ADR-010). Pinned roles never escalate. Nothing to raise on the last
    # iteration: max iter exhausted is the reason then.
    if [ "$i" -lt "$max" ]; then
      esc_rc=0
      orch_tier_escalate "$role" || esc_rc=$?
      if [ "$esc_rc" -eq 1 ]; then
        reason="tier ceiling $(orch_tier_for_role "$role") reached: $fail"
        orch_graph_upsert_node "$node_prefix" worker blocked blocked "$reason" ""
        if type orch_progress_set >/dev/null 2>&1 && [ -n "${ORCH_WORKER_ID:-}" ]; then
          orch_progress_set "$ORCH_WORKER_ID" blocked "$reason"
        else
          printf 'blocked: %s\n' "$reason" >&2
        fi
        return 4
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

  # Fail closed (ADR-006): no verifier prompt means no grounding check, never a pass.
  if [ ! -f "$tpl" ]; then
    printf 'orch: verifier prompt missing (%s); grounding not checked, failing\n' "$tpl" >&2
    return 1
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
  # Empty ORCH_TIER: the worker's escalated/task tier must not leak into the verifier (ADR-010).
  if ORCH_TIER= ORCH_ROLE=verifier orch_agent_run "$wt" "$vprompt"; then
    if [ -f "$wt/.orch/verifier-ok" ]; then
      orch_graph_upsert_node "$vnode" verifier grounding done "grounded" "$parent" agent
      return 0
    fi
  fi
  orch_graph_upsert_node "$vnode" verifier grounding failed "not grounded" "$parent" agent
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
  if ORCH_ROLE=integrator orch_agent_run "$ORCH_REPO" "$out"; then
    orch_graph_upsert_node integrator integrator synthesize done "synthesis complete" "" agent
    return 0
  fi
  orch_graph_upsert_node integrator integrator synthesize failed "integrator agent failed" "" agent
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

# Exit 1 if a task `tier` is not small|medium|large.
orch_graph_plan_validate_tiers() {
  local bad
  bad="$(jq -r '.tasks[] | select(has("tier") and (.tier | IN("small","medium","large") | not)) | "\(.id // "?"): \(.tier)"' "$1")"
  [ -z "$bad" ] && return 0
  printf 'graph-plan: tier must be small, medium or large (task %s)\n' "$(paste -sd, - <<<"$bad")" >&2
  return 1
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
  local tid body paths prompt_file rc_task base baseline bad tier
  tid="$(jq -r ".tasks[$i].id" "$plan_file")"
  # Optional start tier of the task; the loop may still escalate from it (ORCH_TIER_MAX).
  tier="$(jq -r ".tasks[$i].tier // empty" "$plan_file")"
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
  if ! orch_sandbox_wrap bash -c "${tier:+ORCH_TIER=$tier }ORCH_WT=\"$path\" ORCH_WORKER_ID=\"task-${tid}\" ORCH_REPO=\"$ORCH_REPO\" orch_graph_worker \"$path\" \"$prompt_file\""; then
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

  orch_graph_plan_validate_tiers "$plan_file" || return 1

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

  orch_graph_init "${ORCH_RUN_ID:-graph-plan-${issue}-$(date -u +%Y%m%dT%H%M%SZ)}"
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
  ORCH_ROLE=integrator orch_agent_run "$ORCH_REPO" "$out" || return 0
  orch_graph_upsert_node integrator integrator synthesize done "plan synthesis" "" agent
}
