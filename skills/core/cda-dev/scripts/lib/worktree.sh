#!/usr/bin/env bash
# shellcheck shell=bash
# Source-only library (no main).

orch_worktree_path() {
  local repo="$1" id="$2" slug="$3"
  printf '%s/.orch/worktrees/%s-%s\n' "$repo" "$id" "$slug"
}

orch_worktree_add() {
  local repo="$1" id="$2" slug="$3" branch="$4" base="${5:-HEAD}"
  local path cur
  path="$(orch_worktree_path "$repo" "$id" "$slug")"
  mkdir -p "$(dirname "$path")"
  if [ -e "$path" ]; then
    # AFK retry: reuse if this path is already the expected branch worktree.
    if [ -d "$path/.git" ] || [ -f "$path/.git" ]; then
      cur="$(git -C "$path" branch --show-current 2>/dev/null || true)"
      if [ "$cur" = "$branch" ]; then
        printf 'worktree reuse: %s (%s)\n' "$path" "$branch" >&2
        return 0
      fi
    fi
    printf 'worktree stale; replacing: %s\n' "$path" >&2
    git -C "$repo" worktree remove --force "$path" 2>/dev/null || rm -rf "$path"
  fi
  if git -C "$repo" show-ref --verify --quiet "refs/heads/$branch"; then
    git -C "$repo" worktree add "$path" "$branch"
  else
    git -C "$repo" worktree add -b "$branch" "$path" "$base"
  fi
}

orch_worktree_bootstrap() {
  local repo="$1" wt="$2"
  # Secrets: copy only when explicitly requested (ORCH_COPY_ENV=1).
  if [ "${ORCH_COPY_ENV:-0}" = "1" ] && [ -f "$repo/.env" ] && [ ! -e "$wt/.env" ]; then
    cp "$repo/.env" "$wt/.env"
  fi
}

orch_worktree_remove() {
  local repo="$1" wt="$2"
  git -C "$repo" worktree remove --force "$wt"
}
