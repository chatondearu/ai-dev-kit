#!/usr/bin/env bash
# shellcheck shell=bash

orch_detect_agent() {
  if [ -n "${ORCH_AGENT:-}" ]; then
    export ORCH_AGENT_BIN=$ORCH_AGENT
    printf 'custom\n'
    return 0
  fi
  local c
  for c in agent cursor-agent claude opencode; do
    if command -v "$c" >/dev/null 2>&1; then
      export ORCH_AGENT_BIN=$c
      case "$c" in
        agent|cursor-agent) printf 'cursor\n';;
        *) printf '%s\n' "$c";;
      esac
      return 0
    fi
  done
  printf 'No agent CLI found (tried agent, cursor-agent, claude, opencode). Set ORCH_AGENT.\n' >&2
  return 1
}

orch_agent_run() {
  local wt="$1" prompt_file="$2"
  if [ -n "${ORCH_AGENT:-}" ]; then
    export ORCH_AGENT_BIN="$ORCH_AGENT"
  elif [ -z "${ORCH_AGENT_BIN:-}" ]; then
    orch_detect_agent >/dev/null
  fi
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
    agent|cursor-agent)
      (cd "$wt" && "${bin[@]}" -p <"$prompt_file")
      ;;
    *)
      (cd "$wt" && "${bin[@]}" <"$prompt_file")
      ;;
  esac
}
