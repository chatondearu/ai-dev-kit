#!/usr/bin/env bash
# shellcheck shell=bash

# Agent CLI for role $1 (worker|reviewer|verifier|integrator|pr-fix), printed as
# "<cli>\t<source>". Order: ORCH_AGENT_<ROLE> (role-override) > ORCH_AGENT (override)
# > CLI of the session that launched orch (parent) > first CLI on PATH (path). ADR-010.
orch_resolve_agent() {
  local role="${1:-worker}" var c
  var="ORCH_AGENT_${role^^}"
  var="${var//-/_}"
  if [ -n "${!var:-}" ]; then
    printf '%s\t%s\n' "${!var}" role-override
  elif [ -n "${ORCH_AGENT:-}" ]; then
    printf '%s\t%s\n' "$ORCH_AGENT" override
  elif c="$(orch_parent_agent)" && command -v "$c" >/dev/null 2>&1; then
    printf '%s\t%s\n' "$c" parent
  else
    for c in agent cursor-agent claude opencode; do
      if command -v "$c" >/dev/null 2>&1; then
        printf '%s\t%s\n' "$c" path
        return 0
      fi
    done
    printf 'No agent CLI found (tried agent, cursor-agent, claude, opencode). Set ORCH_AGENT.\n' >&2
    return 1
  fi
}

# Agent CLI of the session running us: env markers first, else the nearest ancestor
# process named like a known CLI. ORCH_PS overrides `ps` (tests feed a fake chain).
orch_parent_agent() {
  if [ "${CLAUDECODE:-}" = 1 ]; then echo claude; return 0; fi
  if [ -n "${CODEX_THREAD_ID:-}${CODEX_SESSION_ID:-}" ]; then echo codex; return 0; fi
  # Set by the cursor-agent launcher script (its process comm is `node`).
  case "${CURSOR_INVOKED_AS:-}" in agent|cursor-agent) echo "$CURSOR_INVOKED_AS"; return 0 ;; esac
  local pid="$PPID" ppid comm a1 a2 t n=0
  while [ "${pid:-0}" -gt 1 ] && [ "$n" -lt 64 ]; do
    read -r ppid comm < <(${ORCH_PS:-ps} -o ppid=,comm= -p "$pid" 2>/dev/null) || return 1
    a1="" a2=""
    read -r a1 a2 _ < <(${ORCH_PS:-ps} -o args= -p "$pid" 2>/dev/null) || true
    # comm, then the interpreter and script of args (e.g. `bash /x/bin/agent`).
    for t in "$comm" "$a1" "$a2"; do
      case "${t##*/}" in
        claude|agent|cursor-agent|opencode|codex) echo "${t##*/}"; return 0 ;;
      esac
    done
    pid="$ppid"
    n=$((n + 1))
  done
  return 1
}

# --dry-run: one `agent[<role>]: <cli> (<source>)` line per role.
orch_print_agents() {
  local role r
  for role in worker reviewer verifier integrator pr-fix; do
    r="$(orch_resolve_agent "$role" 2>/dev/null)" || r="none"$'\t'"unresolved"
    printf 'agent[%s]: %s (%s)\n' "$role" "${r%%$'\t'*}" "${r#*$'\t'}"
  done
}

# Kept for headless.sh: prints the kind (custom|cursor|claude|opencode|codex) and
# exports ORCH_AGENT_BIN.
orch_detect_agent() {
  local r
  r="$(orch_resolve_agent worker)" || return 1
  export ORCH_AGENT_BIN="${r%%$'\t'*}"
  if [ -n "${ORCH_AGENT:-}" ]; then
    printf 'custom\n'
    return 0
  fi
  case "$ORCH_AGENT_BIN" in
    agent|cursor-agent) printf 'cursor\n';;
    *) printf '%s\n' "$ORCH_AGENT_BIN";;
  esac
}

# Run the agent CLI of role ORCH_ROLE (default worker) on prompt file $2 in worktree $1.
orch_agent_run() {
  local wt="$1" prompt_file="$2" r
  r="$(orch_resolve_agent "${ORCH_ROLE:-worker}")" || return 1
  export ORCH_AGENT_BIN="${r%%$'\t'*}"
  if [ -n "${ORCH_WORKER_ID:-}" ] && orch_tmux_on; then
    orch_tmux_agent_run "$wt" "$prompt_file"
    return
  fi
  _orch_agent_exec "$wt" "$prompt_file"
}

# Run $ORCH_AGENT_BIN on a prompt file in worktree $1 (stdout/stderr inherited).
_orch_agent_exec() {
  local wt="$1" prompt_file="$2"
  local bin=( $ORCH_AGENT_BIN )
  local name
  name=$(command -p basename "${bin[0]}")
  case "$name" in
    claude)
      # Non-interactive print mode; flags may need adjust per claude version
      (cd "$wt" && "${bin[@]}" -p --dangerously-skip-permissions <"$prompt_file")
      ;;
    opencode)
      (cd "$wt" && "${bin[@]}" run <"$prompt_file")
      ;;
    codex)
      # `exec -` reads the prompt from stdin; like claude above, no approval prompts.
      (cd "$wt" && "${bin[@]}" exec --dangerously-bypass-approvals-and-sandbox - <"$prompt_file")
      ;;
    agent|cursor-agent)
      # --print for scripts; --force avoids interactive approval hangs in headless
      (cd "$wt" && "${bin[@]}" -p --force "$(cat "$prompt_file")")
      ;;
    *)
      (cd "$wt" && "${bin[@]}" <"$prompt_file")
      ;;
  esac
}

# ORCH_TMUX=1 (opt-in, ADR-002): each worker's agent runs in a detached tmux session
# so cdamux can show and attach to it. Never under --sandbox: the tmux server sits
# outside bwrap, so the agent would escape it.
orch_tmux_on() {
  [ "${ORCH_TMUX:-0}" = 1 ] && [ "${ORCH_SANDBOX:-0}" != 1 ] && command -v tmux >/dev/null 2>&1
}

# Session name for worker $1: orch-<run>-<worker>, tmux-safe. Empty $1 → the run prefix.
orch_tmux_session() {
  printf '%s\n' "$(printf 'orch-%s-%s' "${ORCH_RUN_ID:-run}" "$1" | tr -c 'A-Za-z0-9_-' '_')"
}

# Agent output log of worker $1 (listed as `log` in .orch/run/status.json).
orch_tmux_log() {
  printf '%s/.orch/run/%s.log\n' "${ORCH_REPO:-.}" "$1"
}

# Run the agent in the worker's session, wait for its exit code, append its output to
# the worker log and echo it back on stdout (callers parse it, e.g. the review verdict).
# The session is killed after the agent exits unless ORCH_TMUX_KEEP=1.
orch_tmux_agent_run() {
  local wt="$1" prompt_file="$2"
  local s log rcf runner off rc parent="" bin=( $ORCH_AGENT_BIN )
  s="$(orch_tmux_session "$ORCH_WORKER_ID")"
  log="$(orch_tmux_log "$ORCH_WORKER_ID")"
  mkdir -p "$(dirname "$log")"
  touch "$log"
  if ! tmux has-session -t "=$s:" 2>/dev/null; then
    # Placeholder command so remain-on-exit is set before the agent can exit.
    if ! tmux new-session -d -s "$s" -c "$wt" cat 2>/dev/null; then
      printf 'orch: tmux new-session %s failed; running the agent inline\n' "$s" >&2
      _orch_agent_exec "$wt" "$prompt_file"
      return
    fi
    tmux set-option -w -t "=$s:" remain-on-exit on >/dev/null
  fi
  if [ -n "${TMUX:-}" ]; then
    parent="$(tmux display-message -p '#{session_name}' 2>/dev/null || true)"
  fi
  tmux set-option -t "=$s:" @cdamux_run "${ORCH_RUN_ID:-run}" \; \
    set-option -t "=$s:" @cdamux_role worker \; \
    set-option -t "=$s:" @cdamux_agent "$(command -p basename "${bin[0]}")" \; \
    set-option -t "=$s:" @cdamux_state running >/dev/null
  [ -z "$parent" ] || tmux set-option -t "=$s:" @cdamux_parent "$parent" >/dev/null

  # The tmux server does not inherit our environment: the runner replays it. It holds
  # secrets (mktemp → 0600) and deletes itself before doing anything else.
  rcf="$(mktemp "${TMPDIR:-/tmp}/orch-agent-rc.XXXXXX")"
  runner="$(mktemp "${TMPDIR:-/tmp}/orch-agent.XXXXXX")"
  {
    printf 'rm -f -- "$0"\n_t=${TMUX-} _tp=${TMUX_PANE-}\n{\n'
    export -p
    printf '} 2>/dev/null\nexport TMUX="$_t" TMUX_PANE="$_tp"\n'
    declare -f _orch_agent_exec
    printf '_orch_agent_exec %q %q 2>&1 | tee -a %q\n' "$wt" "$prompt_file" "$log"
    printf 'echo "${PIPESTATUS[0]}" >%q\n' "$rcf"
  } >"$runner"
  off="$(wc -c <"$log" | tr -d ' ')"
  tmux respawn-pane -k -t "=$s:" "$(printf '%q %q' "$BASH" "$runner")"
  # Done when the rc lands, the pane dies (agent killed) or the session is gone.
  while [ ! -s "$rcf" ] && [ "$(tmux display-message -p -t "=$s:" '#{pane_dead}' 2>/dev/null)" = 0 ]; do
    sleep 0.2
  done
  rc="$(cat "$rcf" 2>/dev/null || true)"
  rm -f "$rcf" "$runner"
  [ -n "$rc" ] || rc=1
  tail -c +"$((off + 1))" "$log"
  if [ "${ORCH_TMUX_KEEP:-0}" = 1 ]; then
    tmux set-option -t "=$s:" @cdamux_state "$([ "$rc" = 0 ] && echo done || echo error)" 2>/dev/null || true
  else
    tmux kill-session -t "=$s:" 2>/dev/null || true
  fi
  return "$rc"
}

# Kill every session of this run (Ctrl-C path: agents must not outlive orch).
orch_tmux_kill_run() {
  orch_tmux_on || return 0
  local p s
  p="$(orch_tmux_session "")"
  tmux list-sessions -F '#{session_name}' 2>/dev/null | while IFS= read -r s; do
    case "$s" in "$p"*) tmux kill-session -t "=$s:" 2>/dev/null || true ;; esac
  done
}
