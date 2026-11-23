#!/usr/bin/env bash
# shellcheck shell=bash
# Source-only library (no main). ADR-016 allowlist predicate + optional gh pr merge.
# orch_merge_pr is callable outside orch.sh (e.g. foam-board.sh reconcile): source
# lib/worktree.sh, lib/lock.sh, lib/verify.sh, lib/ci.sh and this file; ORCH_REPO and
# ORCH_DEFAULT_BRANCH are derived from the worktree when unset.
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

# Merge-base of HEAD vs origin/<default> (default resolved for this worktree when
# ORCH_DEFAULT_BRANCH is unset, never assumed). Empty → caller fail-closed.
orch_meta_merge_base() {
  local wt="$1" def
  def="${ORCH_DEFAULT_BRANCH:-$(orch_default_branch "$wt")}" || def=""
  [ -n "$def" ] || return 0
  git -C "$wt" merge-base HEAD "origin/$def" 2>/dev/null || true
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

# After verify + review approved + checks green-or-none: merge $1 (worktree $2,
# branch $3) if the GitHub PR and worktree HEAD are allowlisted (ADR-016).
# 0 = merged, 1 = skipped (not allowlisted, PR left open), 2 = merge failed
# (reason in ORCH_MERGE_ERROR and on stderr; the caller reports an error, Q4).
orch_meta_try_merge() {
  local pr="$1" wt="$2" branch="${3:-}" here="$PWD" rc=0
  ORCH_MERGE_ERROR=""
  [ -n "$pr" ] && [ -n "$wt" ] && [ -d "$wt" ] || return 1
  # Every gh call resolves the repo from the worktree, never an inherited GH_REPO.
  local -x GH_REPO=""
  cd "$wt" || return 1
  _orch_meta_try_merge_in_wt "$pr" "$wt" "$branch" || rc=$?
  cd "$here" || true
  return "$rc"
}

_orch_meta_try_merge_in_wt() {
  local pr="$1" wt="$2" branch="$3" pr_paths
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
  [ -n "$branch" ] || branch="$(git -C "$wt" branch --show-current 2>/dev/null)"
  orch_merge_pr "$pr" "$wt" "$branch" || return 2
  printf 'meta-merge: merged PR #%s (ADR-016 allowlist)\n' "$pr" >&2
  return 0
}

_orch_merge_fail() {
  ORCH_MERGE_ERROR="$2"
  printf 'merge: PR #%s not merged: %s\n' "$1" "$2" >&2
  return 1
}

# The one pre-merge rule (Q4/Q5/Q26, shared with skill cda-orchestrator): rebase merges
# only (no fallback); under the per-repo merge lock fetch origin/<default>; when the
# branch is behind, rebase it, re-run verify, push (lease on the old head) and re-watch
# checks; refuse mergeable=UNKNOWN; merge pinned to the verified HEAD. 0 = merged.
orch_merge_pr() {
  local pr="$1" wt="$2" branch="$3" here="$PWD" rc=0
  ORCH_MERGE_ERROR=""
  [ -d "$wt" ] || { _orch_merge_fail "$pr" "worktree $wt missing"; return 1; }
  # Every gh call (repo view, pr view, checks, mergeable, merge) resolves the repo
  # from the worktree, never an inherited GH_REPO pointing elsewhere.
  local -x GH_REPO=""
  cd "$wt" || { _orch_merge_fail "$pr" "cannot cd to $wt"; return 1; }
  _orch_merge_pr_in_wt "$pr" "$wt" "$branch" || rc=$?
  cd "$here" || true
  return "$rc"
}

_orch_merge_pr_in_wt() {
  local pr="$1" wt="$2" branch="$3" def allowed head_ref rc=0
  local ORCH_REPO="${ORCH_REPO:-$(orch_repo_root "$wt")}"
  def="${ORCH_DEFAULT_BRANCH:-}"
  [ -n "$def" ] || def="$(orch_default_branch "$wt")" \
    || { _orch_merge_fail "$pr" "cannot resolve the default branch of origin"; return 1; }
  [ -n "$branch" ] && [ "$branch" != "$def" ] \
    || { _orch_merge_fail "$pr" "head branch '${branch}' is empty or the default branch"; return 1; }
  allowed="$(gh repo view --json rebaseMergeAllowed -q .rebaseMergeAllowed 2>/dev/null)" || allowed=""
  [ "$allowed" = true ] || {
    _orch_merge_fail "$pr" "the repo does not allow rebase merges (rebaseMergeAllowed=${allowed:-unreadable}); enable it or merge by hand"
    return 1
  }
  # The leased push below rewrites $branch: it must be this PR's head, not a guess.
  head_ref="$(gh pr view "$pr" --json headRefName -q .headRefName 2>/dev/null)" || head_ref=""
  [ "$head_ref" = "$branch" ] || {
    _orch_merge_fail "$pr" "PR head is '${head_ref:-unreadable}', not $branch"
    return 1
  }
  orch_merge_lock_acquire \
    || { _orch_merge_fail "$pr" "merge lock $ORCH_REPO/.orch/merge.lock still held (ORCH_MERGE_LOCK_WAIT)"; return 1; }
  _orch_merge_pr_locked "$pr" "$wt" "$branch" "$def" || rc=1
  orch_merge_lock_release
  return "$rc"
}

# Commands here close fd 9: a daemon started by verify must not keep the flock (Linux).
_orch_merge_pr_locked() {
  local pr="$1" wt="$2" branch="$3" def="$4" old head st m err
  git -C "$wt" fetch -q origin "$def" 9>&- \
    || { _orch_merge_fail "$pr" "git fetch origin $def failed"; return 1; }
  if ! git -C "$wt" merge-base --is-ancestor "origin/$def" HEAD; then
    printf 'merge: PR #%s is behind origin/%s; rebasing and re-running verify\n' "$pr" "$def" >&2
    old="$(git -C "$wt" rev-parse HEAD)" || { _orch_merge_fail "$pr" "cannot read HEAD in $wt"; return 1; }
    if [ -n "$(git -C "$wt" status --porcelain --untracked-files=no)" ]; then
      _orch_merge_fail "$pr" "worktree has uncommitted changes; cannot rebase on origin/$def"
      return 1
    fi
    if ! git -C "$wt" rebase -q "origin/$def" >&2 9>&-; then
      git -C "$wt" rebase --abort 2>/dev/null 9>&- || true
      _orch_merge_fail "$pr" "rebase on origin/$def conflicts; rebase by hand"
      return 1
    fi
    mkdir -p "$wt/.orch"
    orch_run_verify "$wt" >"$wt/.orch/merge-verify.log" 2>&1 9>&- \
      || { _orch_merge_fail "$pr" "verify failed after rebase on origin/$def (see $wt/.orch/merge-verify.log)"; return 1; }
    git -C "$wt" push -q --force-with-lease="$branch:$old" origin "HEAD:$branch" 9>&- \
      || { _orch_merge_fail "$pr" "push of the rebased $branch failed"; return 1; }
    st="$(orch_pr_checks_watch "$pr" 9>&-)" \
      || { _orch_merge_fail "$pr" "checks ${st:-unknown} after rebase"; return 1; }
  fi
  m="$(orch_pr_mergeable "$pr" 9>&-)" \
    || { _orch_merge_fail "$pr" "not mergeable (mergeable=${m:-UNKNOWN})"; return 1; }
  head="$(git -C "$wt" rev-parse HEAD)" || { _orch_merge_fail "$pr" "cannot read HEAD in $wt"; return 1; }
  if ! err="$(GH_PROMPT_DISABLED=1 gh pr merge "$pr" --rebase --match-head-commit "$head" </dev/null 2>&1 9>&-)"; then
    _orch_merge_fail "$pr" "gh pr merge --rebase failed: $(printf '%s' "$err" | tail -n 1)"
    return 1
  fi
}

# foam-board.sh of the cda-kanban skill: ORCH_FOAM_BOARD, the kit sibling, else an installed copy.
orch_foam_board_path() {
  local c
  for c in "${ORCH_FOAM_BOARD:-}" \
    "${ORCH_ROOT:-}/../../cda-kanban/scripts/foam-board.sh" \
    "${HOME}/.claude/skills/cda-kanban/scripts/foam-board.sh" \
    "${HOME}/.cursor/skills/cda-kanban/scripts/foam-board.sh"; do
    if [ -n "$c" ] && [ -f "$c" ]; then
      printf '%s\n' "$c"
      return 0
    fi
  done
  return 1
}

# Board truth (ADR-016): cards whose PR merged → done through a foam-only PR that
# self-merges (`foam-board.sh reconcile --pr`). Never fails the run. Off with
# ORCH_BOARD_RECONCILE=0 or when the repo has no foam/board.
orch_board_reconcile() {
  local repo="$1" fb
  [ "${ORCH_BOARD_RECONCILE:-1}" != 0 ] && [ -d "$repo/foam/board" ] || return 0
  if ! fb="$(orch_foam_board_path)"; then
    printf 'orch: board reconcile skipped (foam-board.sh not found)\n' >&2
    return 0
  fi
  (cd "$repo" && bash "$fb" reconcile --pr) >&2 \
    || printf 'orch: board reconcile failed; run `foam-board.sh reconcile --pr` in %s\n' "$repo" >&2
}
