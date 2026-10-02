#!/usr/bin/env bash
# shellcheck shell=bash
# Source-only library (no main).

orch_resolve_verify() {
  local dir="$1"
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
