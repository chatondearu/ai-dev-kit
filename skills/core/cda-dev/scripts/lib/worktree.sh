#!/usr/bin/env bash
# shellcheck shell=bash
# Source-only library (no main).

orch_worktree_path() {
  local repo="$1" id="$2" slug="$3"
  printf '%s/.orch/worktrees/%s-%s\n' "$repo" "$id" "$slug"
}

orch_worktree_add() {
  local repo="$1" id="$2" slug="$3" branch="$4" base="${5:-HEAD}"
  local path
  path="$(orch_worktree_path "$repo" "$id" "$slug")"
  mkdir -p "$(dirname "$path")"
  if [ -e "$path" ]; then
    printf 'worktree already exists: %s\n' "$path" >&2
    return 1
  fi
  git -C "$repo" worktree add -b "$branch" "$path" "$base"
}

orch_worktree_bootstrap() {
  local repo="$1" wt="$2"
  if [ -f "$repo/.env" ] && [ ! -e "$wt/.env" ]; then
    cp "$repo/.env" "$wt/.env"
  fi
}

orch_worktree_remove() {
  local repo="$1" wt="$2"
  git -C "$repo" worktree remove --force "$wt"
}
