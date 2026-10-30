#!/usr/bin/env bash
# shellcheck shell=bash

orch_sandbox_wrap() {
  if [ "${ORCH_SANDBOX:-0}" != "1" ]; then
    "$@"
    return $?
  fi
  if ! command -v bwrap >/dev/null 2>&1; then
    printf 'error: --sandbox requested but bwrap missing; refusing to continue unsandboxed\n' >&2
    return 1
  fi
  local wt="${ORCH_WT:?ORCH_WT required for sandbox}"
  local args=(--die-with-parent --new-session)
  local repo_orch=""
  args+=(--bind "$wt" "$wt" --chdir "$wt")
  # Workers write live graph / status under ORCH_REPO/.orch (often outside the worktree).
  if [ -n "${ORCH_REPO:-}" ]; then
    repo_orch="${ORCH_REPO}/.orch"
    mkdir -p "$repo_orch/graph" "$repo_orch/run" 2>/dev/null || true
    if [ -d "$repo_orch" ]; then
      case "$repo_orch" in
        "$wt"|"$wt"/*) ;;
        *) args+=(--bind "$repo_orch" "$repo_orch") ;;
      esac
    fi
  fi
  for p in /usr /bin /lib /lib64 /nix /etc/ssl /etc/resolv.conf; do
    [ -e "$p" ] && args+=(--ro-bind "$p" "$p")
  done
  if [ -n "${ORCH_PROMPTS_DIR:-}" ] && [ -d "$ORCH_PROMPTS_DIR" ]; then
    args+=(--ro-bind "$ORCH_PROMPTS_DIR" "$ORCH_PROMPTS_DIR")
  fi
  args+=(--dev /dev --proc /proc --tmpfs /tmp)
  bwrap "${args[@]}" "$@"
}
