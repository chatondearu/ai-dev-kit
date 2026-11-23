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

# KiB a checkout of REPO's HEAD writes (tracked blob sizes; submodules count 0).
orch_checkout_kib() {
  git -C "$1" ls-tree -r -l HEAD 2>/dev/null | awk '{ s += $4 } END { print int(s / 1024) + 1 }'
}

# Refuse new worktrees when DIR's filesystem has less than ORCH_MIN_FREE_GIB free
# (default 40, 0 = off) plus, given REPO, the checkout about to be created.
# Fails closed when df is unreadable (ADR-017).
orch_disk_guard() {
  local dir="$1" repo="${2:-}" min="${ORCH_MIN_FREE_GIB:-40}" avail checkout=0
  [ "$min" -gt 0 ] 2>/dev/null || return 0
  while [ ! -d "$dir" ] && [ "$dir" != / ]; do dir="$(dirname "$dir")"; done
  avail="$(df -Pk "$dir" 2>/dev/null | awk 'NR==2 { for (i = 2; i <= NF; i++) if ($i ~ /%$/) { print $(i-1); exit } }')"
  case "$avail" in '' | *[!0-9]*)
    printf 'disk guard: cannot read free space on %s (df failed); refusing to create a worktree (ORCH_MIN_FREE_GIB=0 turns the guard off)\n' "$dir" >&2
    return 1 ;;
  esac
  [ -z "$repo" ] || checkout="$(orch_checkout_kib "$repo")"
  [ "$avail" -lt $((min * 1048576 + checkout)) ] || return 0
  printf 'low disk: %s GiB free on %s (< %s GiB, ORCH_MIN_FREE_GIB, + %s MiB for the checkout); free build caches with `cdamux worker clean --dry-run` (add --root <repo>/.orch/worktrees)\n' \
    "$((avail / 1048576))" "$dir" "$min" "$((checkout / 1024))" >&2
  return 1
}

# Free build dirs of finished orch worktrees (PR merged/closed) via `cdamux worker clean`.
# No-op without cdamux on PATH; never fails the run (ADR-017).
orch_free_build_dirs() {
  local root="$1/.orch/worktrees" wt
  [ -d "$root" ] && command -v cdamux >/dev/null 2>&1 || return 0
  # Only orch.sh creates worktrees here: adopt ones made before the marker existed.
  for wt in "$root"/*; do orch_worktree_mark "$wt"; done
  cdamux worker clean --root "$root" || true
}

# Mark linked worktree $1 as orch-created: `cdamux worker clean` only frees build dirs
# of marked or registry-known worktrees (ADR-017). The marker lives in the worktree's
# own git dir, so it never shows in `git status` and goes away with the worktree.
orch_worktree_mark() {
  local d
  [ -f "$1/.git" ] || return 0
  d="$(git -C "$1" rev-parse --absolute-git-dir 2>/dev/null)" && [ -d "$d" ] || return 0
  : >"$d/cdamux-managed" 2>/dev/null || true
}

# Default branch of DIR's origin (main, dev, develop…), never assumed (Q4):
# the remote's HEAD, else gh, else the cached origin/HEAD. Fails when unknown.
orch_default_branch() {
  local dir="$1" b
  b="$(git -C "$dir" ls-remote --symref origin HEAD 2>/dev/null \
    | awk '$1 == "ref:" { sub("^refs/heads/", "", $2); print $2; exit }')"
  if [ -z "$b" ] && command -v gh >/dev/null 2>&1; then
    b="$(cd "$dir" && gh repo view --json defaultBranchRef -q .defaultBranchRef.name 2>/dev/null)" || b=""
  fi
  if [ -z "$b" ]; then
    b="$(git -C "$dir" symbolic-ref --quiet --short refs/remotes/origin/HEAD 2>/dev/null)" || b=""
    b="${b#origin/}"
  fi
  case "$b" in '' | *[[:space:]{}]*) return 1 ;; esac
  printf '%s\n' "$b"
}

# Fetch DIR's default branch and export ORCH_DEFAULT_BRANCH + ORCH_BASE (origin/<default>)
# so worktrees start from the remote, not a stale local HEAD (Q11, H6). A repo with no
# origin remote has nothing to fetch: ORCH_BASE=HEAD. Fails closed when origin exists
# but the branch cannot be resolved or fetched.
orch_sync_default() {
  local dir="$1"
  if ! git -C "$dir" remote get-url origin >/dev/null 2>&1; then
    export ORCH_DEFAULT_BRANCH="" ORCH_BASE=HEAD
    return 0
  fi
  ORCH_DEFAULT_BRANCH="$(orch_default_branch "$dir")" || {
    printf 'orch: cannot resolve the default branch of origin in %s\n' "$dir" >&2
    return 1
  }
  if ! git -C "$dir" fetch -q origin "$ORCH_DEFAULT_BRANCH"; then
    printf 'orch: git fetch origin %s failed; refusing to start from a stale base\n' "$ORCH_DEFAULT_BRANCH" >&2
    return 1
  fi
  export ORCH_DEFAULT_BRANCH ORCH_BASE="origin/$ORCH_DEFAULT_BRANCH"
}

# Branch name prompts diff and rebase against: the fetched default branch, else (no
# origin remote) the main checkout's current branch.
orch_base_branch_name() {
  printf '%s\n' "${ORCH_DEFAULT_BRANCH:-$(git -C "${ORCH_REPO:-.}" branch --show-current 2>/dev/null)}"
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
    # --no-track: a base of origin/<default> must not become the feature branch's upstream.
    git -C "$repo" worktree add --no-track -b "$branch" "$path" "$base"
  fi
}

orch_worktree_bootstrap() {
  local repo="$1" wt="$2"
  orch_worktree_mark "$wt"
  # Secrets: copy only when explicitly requested (ORCH_COPY_ENV=1).
  if [ "${ORCH_COPY_ENV:-0}" = "1" ] && [ -f "$repo/.env" ] && [ ! -e "$wt/.env" ]; then
    cp "$repo/.env" "$wt/.env"
  fi
}

orch_worktree_remove() {
  local repo="$1" wt="$2"
  git -C "$repo" worktree remove --force "$wt"
}
