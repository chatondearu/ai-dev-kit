#!/usr/bin/env bash
# shellcheck shell=bash
# Source-only library (no main).

# Main checkout root for DIR, even from inside a linked worktree, so `.orch/` is
# never created inside a worktree (nested worktrees). Non-git DIR: its own path.
orch_repo_root() {
  local top common
  top="$(git -C "$1" rev-parse --show-toplevel 2>/dev/null)" || { (cd "$1" && pwd); return; }
  common="$(cd "$top" && cd "$(git rev-parse --git-common-dir)" && pwd)"
  if [ "$(cd "$top" && cd "$(git rev-parse --git-dir)" && pwd)" != "$common" ]; then
    dirname "$common"
  else
    printf '%s\n' "$top"
  fi
}

orch_worktree_path() {
  local repo="$1" id="$2" slug="$3"
  printf '%s/.orch/worktrees/%s-%s\n' "$repo" "$id" "$slug"
}

orch_worktree_add() {
  local repo="$1" id="$2" slug="$3" branch="$4" base="${5:-HEAD}"
  local path cur
  path="$(orch_worktree_path "$repo" "$id" "$slug")"
  mkdir -p "$(dirname "$path")"
  # Drop registrations of worktree dirs deleted by hand ("missing but already registered").
  git -C "$repo" worktree prune 2>/dev/null || true
  if [ -e "$path" ]; then
    if [ -d "$path/.git" ] || [ -f "$path/.git" ]; then
      cur="$(git -C "$path" branch --show-current 2>/dev/null || true)"
      # AFK retry: reuse if this path is already the expected branch worktree.
      if [ "$cur" = "$branch" ]; then
        printf 'worktree reuse: %s (%s)\n' "$path" "$branch" >&2
        return 0
      fi
      # Detached HEAD (e.g. blocked mid-rebase) or uncommitted files: that is work, never delete it.
      if [ -z "$cur" ] || [ -n "$(git -C "$path" status --porcelain 2>/dev/null)" ]; then
        printf 'worktree has a detached HEAD or uncommitted changes; not replacing: %s\n' "$path" >&2
        return 1
      fi
      printf 'worktree stale; replacing: %s\n' "$path" >&2
      git -C "$repo" worktree remove "$path" || return 1
    else
      printf 'worktree stale; replacing: %s\n' "$path" >&2
      rm -rf "$path"
    fi
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
