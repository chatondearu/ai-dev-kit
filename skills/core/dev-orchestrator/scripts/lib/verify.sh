#!/usr/bin/env bash
# shellcheck shell=bash
# Source-only library (no main).

_orch_extract_agents_verify() {
  local file="$1"
  [ -f "$file" ] || return 1
  local match
  # 1. line like "verify: <cmd>" (case-insensitive)
  match="$(grep -m 1 -Ei '^[[:space:]]*(-[[:space:]]*)?verify[[:space:]]*:' "$file" | head -n 1 || true)"
  if [ -n "$match" ]; then
    local cmd
    cmd="$(echo "$match" | sed -E 's/^[[:space:]]*(-[[:space:]]*)?verify[[:space:]]*:[[:space:]]*//I' | tr -d '`' | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//')"
    if [ -n "$cmd" ]; then
      printf '%s\n' "$cmd"
      return 0
    fi
  fi
  # 2. command inside fenced code block (``` ... ```)
  match="$(awk '/^```/ { in_fence = !in_fence; next } in_fence && /(nix develop|npm test|pnpm test)/ { gsub(/^[ \t]+|[ \t]+$/, ""); print; exit }' "$file" || true)"
  if [ -n "$match" ]; then
    printf '%s\n' "$match"
    return 0
  fi
  # 3. inline fenced `nix develop...`, `npm test`, or `pnpm test`
  match="$(grep -m 1 -oE '`(nix develop[^`]*|npm test|pnpm test)`' "$file" | head -n 1 | tr -d '`' || true)"
  if [ -n "$match" ]; then
    printf '%s\n' "$match"
    return 0
  fi
  return 1
}

orch_resolve_verify() {
  local dir="$1"
  if [ -f "$dir/AGENTS.md" ]; then
    local agents_cmd
    if agents_cmd="$(_orch_extract_agents_verify "$dir/AGENTS.md")" && [ -n "$agents_cmd" ]; then
      printf '%s\n' "$agents_cmd"
      return 0
    fi
  fi
  if [ -f "$dir/flake.nix" ] || [ -f "$dir/.envrc" ]; then
    if [ -f "$dir/package.json" ]; then
      printf 'nix develop -c npm test\n'
      return 0
    fi
    if [ -f "$dir/Makefile" ]; then
      printf 'nix develop -c make test\n'
      return 0
    fi
  fi
  if [ -f "$dir/pnpm-lock.yaml" ]; then
    printf 'pnpm test\n'
    return 0
  fi
  if [ -f "$dir/package.json" ]; then
    printf 'npm test\n'
    return 0
  fi
  if [ -f "$dir/Makefile" ]; then
    printf 'make test\n'
    return 0
  fi
  if [ -f "$dir/Cargo.toml" ]; then
    printf 'cargo test\n'
    return 0
  fi
  printf 'orch_resolve_verify: no verify command heuristic matched; using true\n' >&2
  printf 'true\n'
}

orch_run_verify() {
  local dir="$1"
  local cmd
  cmd="$(orch_resolve_verify "$dir")"
  (cd "$dir" && eval "$cmd")
}
