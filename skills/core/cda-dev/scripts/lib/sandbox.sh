#!/usr/bin/env bash
# shellcheck shell=bash

orch_sandbox_wrap() {
  if [ "${ORCH_SANDBOX:-0}" != "1" ]; then
    "$@"
    return $?
  fi
  if ! command -v bwrap >/dev/null 2>&1; then
    printf 'warn: --sandbox requested but bwrap missing; continuing unsandboxed\n' >&2
    "$@"
    return $?
  fi
  local wt="${ORCH_WT:?ORCH_WT required for sandbox}"
  local args=(--die-with-parent --new-session)
  args+=(--bind "$wt" "$wt" --chdir "$wt")
  for p in /usr /bin /lib /lib64 /nix /etc/ssl /etc/resolv.conf; do
    [ -e "$p" ] && args+=(--ro-bind "$p" "$p")
  done
  if [ -n "${ORCH_PROMPTS_DIR:-}" ] && [ -d "$ORCH_PROMPTS_DIR" ]; then
    args+=(--ro-bind "$ORCH_PROMPTS_DIR" "$ORCH_PROMPTS_DIR")
  fi
  args+=(--dev /dev --proc /proc --tmpfs /tmp)
  bwrap "${args[@]}" "$@"
}
