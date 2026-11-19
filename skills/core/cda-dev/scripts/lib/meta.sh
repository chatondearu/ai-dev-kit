#!/usr/bin/env bash
# shellcheck shell=bash
# Source-only library (no main). ADR-016 allowlist predicate + optional gh pr merge.
# Never --admin, never force-push. Empty diffs fail closed.

# True when $1 is an ADR-016 allowlist path (prefix rules). Fail closed otherwise.
# foam/features/ is a product catalog (not "foam that does not change product status").
orch_meta_path_allowed() {
  local p="${1#./}"
  case "$p" in
    "") return 1 ;;
    foam/features|foam/features/*) return 1 ;;
    prd|prd/*) return 0 ;;
    plans|plans/*) return 0 ;;
    foam/board|foam/board/*) return 0 ;;
    foam/decisions|foam/decisions/*) return 0 ;;
    foam|foam/*) return 0 ;;
    *) return 1 ;;
  esac
}

# stdin: one path per line (git diff --name-only). True iff at least one path
# and every path is allowlisted.
orch_meta_paths_allowlisted() {
  local p any=0
  while IFS= read -r p || [ -n "$p" ]; do
    [ -n "$p" ] || continue
    any=1
    orch_meta_path_allowed "$p" || return 1
  done
  [ "$any" = 1 ]
}

# Merge-base of HEAD vs origin/main (then main/master). Empty → caller fail-closed.
orch_meta_merge_base() {
  local wt="$1"
  git -C "$wt" merge-base HEAD origin/main 2>/dev/null \
    || git -C "$wt" merge-base HEAD origin/master 2>/dev/null \
    || git -C "$wt" merge-base HEAD main 2>/dev/null \
    || git -C "$wt" merge-base HEAD master 2>/dev/null \
    || true
}

# True when every committed path merge-base…HEAD in $1 is allowlisted.
# --no-renames so a rename's old path cannot hide outside the list.
orch_meta_diff_allowlisted() {
  local wt="$1" base paths
  [ -n "$wt" ] && [ -d "$wt" ] || return 1
  base="$(orch_meta_merge_base "$wt")"
  [ -n "$base" ] || return 1
  paths="$(git -C "$wt" diff --name-only --no-renames "$base" HEAD)" || return 1
  printf '%s\n' "$paths" | orch_meta_paths_allowlisted
}

# Paths GitHub will merge: name-only plus `rename from` (old path on rename).
orch_meta_pr_diff_paths() {
  local pr="$1" names patch
  [ -n "$pr" ] || return 1
  names="$(gh pr diff "$pr" --name-only --color=never)" || return 1
  patch="$(gh pr diff "$pr" --color=never)" || patch=""
  {
    printf '%s\n' "$names"
    printf '%s\n' "$patch" | sed -n 's/^rename from //p'
  } | sed '/^$/d'
}

# After verify + REVIEW: approved + checks green-or-none: merge $1 if the
# GitHub PR (and worktree HEAD) are allowlisted. 0 = merged, 1 = skipped or
# gh failed (PR left open). --match-head-commit refuses a different remote SHA.
# Repo default merge method (no --merge/--squash/--rebase/--admin).
orch_meta_try_merge() {
  local pr="$1" wt="$2" pr_paths head
  [ -n "$pr" ] && [ -n "$wt" ] || return 1
  pr_paths="$(orch_meta_pr_diff_paths "$pr")" || {
    printf 'meta-merge: skip PR #%s (could not list PR files)\n' "$pr" >&2
    return 1
  }
  if ! printf '%s\n' "$pr_paths" | orch_meta_paths_allowlisted; then
    printf 'meta-merge: skip PR #%s (PR diff outside ADR-016 allowlist)\n' "$pr" >&2
    return 1
  fi
  if ! orch_meta_diff_allowlisted "$wt"; then
    printf 'meta-merge: skip PR #%s (worktree diff outside ADR-016 allowlist)\n' "$pr" >&2
    return 1
  fi
  head="$(git -C "$wt" rev-parse HEAD)" || return 1
  if ! GH_PROMPT_DISABLED=1 gh pr merge "$pr" --match-head-commit "$head" </dev/null; then
    printf 'meta-merge: gh pr merge failed for PR #%s (PR left open)\n' "$pr" >&2
    return 1
  fi
  printf 'meta-merge: merged PR #%s (ADR-016 allowlist)\n' "$pr" >&2
  return 0
}
