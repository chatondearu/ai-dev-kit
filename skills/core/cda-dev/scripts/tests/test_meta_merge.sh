#!/usr/bin/env bash
# ADR-016 allowlist predicate + harness merge stub (#406).
set -euo pipefail
TROOT="$(cd "$(dirname "$0")" && pwd)"
SCRIPTS="$(cd "$TROOT/.." && pwd)"
# shellcheck source=helpers.sh
source "$TROOT/helpers.sh"
# shellcheck source=../lib/meta.sh
source "$TROOT/../lib/meta.sh"
# shellcheck source=../lib/worktree.sh
source "$TROOT/../lib/worktree.sh"

TMP="$(cd "$(mktemp -d)" && pwd -P)"
_ORIG_PATH="$PATH"
STUB_BIN="$(mktemp -d)"
MERGE_LOG="$STUB_BIN/merge.log"

cleanup() {
  PATH="$_ORIG_PATH"
  rm -rf "$TMP/.orch/worktrees" 2>/dev/null || true
  git -C "$TMP" worktree prune 2>/dev/null || true
  rm -rf "$TMP" "$STUB_BIN"
}
trap cleanup EXIT

paths_rc() {
  set +e
  printf '%s\n' "$@" | orch_meta_paths_allowlisted
  local rc=$?
  set -e
  printf '%s\n' "$rc"
}

echo "# path lists: allow"
assert_eq "$(paths_rc \
  'prd/PRD-403.md' \
  'plans/PLAN-406.md' \
  'foam/decisions/ADR-016-foam-first-kanban.md' \
  'foam/board/406-worker-meta-automerge.md' \
  'foam/index.md' \
  'foam/architecture/notes.md')" "0" \
  "prd+plans+decisions+board+moc allowlisted"

echo "# path lists: reject"
assert_eq "$(paths_rc 'doc/orchestration.md')" "1" "doc/ rejected"
assert_eq "$(paths_rc 'README.md')" "1" "README rejected"
assert_eq "$(paths_rc 'skills/core/cda-dev/SKILL.md')" "1" "skills/ rejected"
assert_eq "$(paths_rc 'foam/features/shipped.md')" "1" "shipped catalog rejected"
assert_eq "$(paths_rc 'foam/features/backlog.md')" "1" "backlog catalog rejected"
assert_eq "$(paths_rc 'foam/features/out-of-scope.md')" "1" "out-of-scope catalog rejected"
assert_eq "$(paths_rc 'foam/features/index.md')" "1" "features index catalog rejected"
assert_eq "$(paths_rc 'prd/x.md' 'skills/x.sh')" "1" "mixed list fail-closed"
assert_eq "$(paths_rc)" "1" "empty list fail-closed"
assert_eq "$(paths_rc 'cdamux/crates/cdamux-cli/src/main.rs')" "1" "cdamux crate rejected"
assert_eq "$(paths_rc 'install.sh')" "1" "install.sh rejected"

echo "# git diff merge-base…HEAD (feature branch vs main)"
git -C "$TMP" init -q
git -C "$TMP" config user.email test@test
git -C "$TMP" config user.name test
echo init >"$TMP/README"
git -C "$TMP" add README
git -C "$TMP" commit -qm init
git -C "$TMP" branch -M main
git -C "$TMP" checkout -q -b feat-allow
mkdir -p "$TMP/prd" "$TMP/plans" "$TMP/foam/board" "$TMP/foam/decisions"
printf 'meta\n' >"$TMP/prd/x.md"
printf 'plan\n' >"$TMP/plans/y.md"
printf 'adr\n' >"$TMP/foam/decisions/z.md"
printf 'card\n' >"$TMP/foam/board/c.md"
git -C "$TMP" add prd plans foam
git -C "$TMP" commit -qm "allowlisted"
if orch_meta_diff_allowlisted "$TMP"; then
  PASSES=$((PASSES + 1))
else
  FAILS=$((FAILS + 1))
  printf 'FAIL allowlisted commit should pass predicate\n' >&2
fi

mkdir -p "$TMP/skills"
printf 'nope\n' >"$TMP/skills/x.sh"
git -C "$TMP" add skills/x.sh
git -C "$TMP" commit -qm "skills"
if orch_meta_diff_allowlisted "$TMP"; then
  FAILS=$((FAILS + 1))
  printf 'FAIL skills path must fail-closed\n' >&2
else
  PASSES=$((PASSES + 1))
fi

git -C "$TMP" reset --hard HEAD~1 >/dev/null

echo "# rename: old path outside allowlist still fail-closed"
git -C "$TMP" checkout -q main
mkdir -p "$TMP/skills"
printf 'old\n' >"$TMP/skills/old.sh"
git -C "$TMP" add skills/old.sh
git -C "$TMP" commit -qm "skills on main"
git -C "$TMP" checkout -q -b feat-rename
mkdir -p "$TMP/prd"
git -C "$TMP" mv skills/old.sh prd/new.md
git -C "$TMP" commit -qm "rename into prd"
if orch_meta_diff_allowlisted "$TMP"; then
  FAILS=$((FAILS + 1))
  printf 'FAIL rename from skills/ to prd/ must fail-closed\n' >&2
else
  PASSES=$((PASSES + 1))
fi
git -C "$TMP" checkout -q feat-allow

echo "# harness stub: merge invoked only on allow"
: >"$MERGE_LOG"
cat >"$STUB_BIN/gh" <<EOF
#!/usr/bin/env bash
echo "\$*" >>"$MERGE_LOG"
case "\$*" in
  *"pr diff"*)
    printf '%s\n' "\${GH_STUB_PR_FILES:-}"
    exit 0
    ;;
  *"pr merge"*)
    exit 0
    ;;
  *)
    exit 0
    ;;
esac
EOF
chmod +x "$STUB_BIN/gh"
export PATH="$STUB_BIN:$PATH"

ALLOW_PR_FILES=$'prd/x.md\nplans/y.md\nfoam/decisions/z.md\nfoam/board/c.md'
set +e
GH_STUB_PR_FILES="$ALLOW_PR_FILES" orch_meta_try_merge 42 "$TMP" >/dev/null 2>"$STUB_BIN/allow.err"
rc_allow=$?
set -e
assert_eq "$rc_allow" "0" "try_merge succeeds on allowlist diff"
assert_contains "$(cat "$MERGE_LOG")" "pr merge 42" "gh pr merge invoked on allow"
assert_contains "$(cat "$MERGE_LOG")" "match-head-commit" "merge pins worktree HEAD"

echo "# local allowlist + stub PR files with skills/ must not merge"
: >"$MERGE_LOG"
set +e
GH_STUB_PR_FILES=$'prd/x.md\nskills/from-worker.sh' \
  orch_meta_try_merge 44 "$TMP" >/dev/null 2>"$STUB_BIN/prdirty.err"
rc_prdirty=$?
set -e
assert_eq "$rc_prdirty" "1" "try_merge skips when GitHub PR files include skills/"
if grep -q "pr merge" "$MERGE_LOG"; then
  FAILS=$((FAILS + 1))
  printf 'FAIL gh pr merge must not run when PR files include skills/\n%s\n' "$(cat "$MERGE_LOG")" >&2
else
  PASSES=$((PASSES + 1))
fi
assert_contains "$(cat "$STUB_BIN/prdirty.err")" "PR diff outside ADR-016 allowlist" "PR-files skip reason logged"

: >"$MERGE_LOG"
mkdir -p "$TMP/doc"
printf 'docs\n' >"$TMP/doc/x.md"
git -C "$TMP" add doc/x.md
git -C "$TMP" commit -qm "doc"
set +e
GH_STUB_PR_FILES="$ALLOW_PR_FILES" orch_meta_try_merge 43 "$TMP" >/dev/null 2>"$STUB_BIN/reject.err"
rc_reject=$?
set -e
assert_eq "$rc_reject" "1" "try_merge skips mixed/doc worktree diff"
if grep -q "pr merge" "$MERGE_LOG"; then
  FAILS=$((FAILS + 1))
  printf 'FAIL gh pr merge must not run on reject\n' >&2
else
  PASSES=$((PASSES + 1))
fi
assert_contains "$(cat "$STUB_BIN/reject.err")" "outside ADR-016 allowlist" "skip reason logged"

echo "# orch.sh ready-pickup: merge only when worker committed allowlist paths"
git -C "$TMP" checkout -q main
git -C "$TMP" branch -D feat-allow >/dev/null 2>&1 || true
printf '{"scripts":{"test":"true"}}\n' >"$TMP/package.json"
git -C "$TMP" add package.json
git -C "$TMP" commit -qm "verify"

cat >"$STUB_BIN/stub-agent" <<'EOF'
#!/usr/bin/env bash
prompt="$(cat)"
mkdir -p .orch
case "$prompt" in
  *"Verifier (grounding node)"*) touch .orch/verifier-ok; exit 0 ;;
  *"Reviewer prompt"*) echo "REVIEW: approved"; exit 0 ;;
esac
if [ "${STUB_COMMIT_META:-0}" = "1" ]; then
  mkdir -p prd
  echo body >prd/from-worker.md
  git add prd/from-worker.md
  git -c user.email=w@t -c user.name=w commit -qm "meta from worker"
elif [ "${STUB_COMMIT_SKILLS:-0}" = "1" ]; then
  mkdir -p skills
  echo body >skills/from-worker.sh
  git add skills/from-worker.sh
  git -c user.email=w@t -c user.name=w commit -qm "skills from worker"
fi
exit 0
EOF
chmod +x "$STUB_BIN/stub-agent"
cat >"$STUB_BIN/npm" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
chmod +x "$STUB_BIN/npm"

cat >"$STUB_BIN/gh" <<EOF
#!/usr/bin/env bash
echo "\$*" >>"$MERGE_LOG"
case "\$*" in
  *"issue view"*)
    case "\$*" in
      *"--comments"*|*"--json comments"*)
        printf '%s\n' '{"comments":[{"authorAssociation":"OWNER","body":"## Agent Brief\n\n**Acceptance criteria:**\n- [ ] done\n"}]}'
        ;;
      *"--json body"*) printf '%s\n' '{"body":"Test"}' ;;
      *"--json title"*) echo "Test" ;;
      *) echo "Test" ;;
    esac
    ;;
  *"pr list"*)
    echo "[{\"number\":\${GH_STUB_PR_NUM:-301},\"state\":\"OPEN\"}]"
    ;;
  *"pr diff"*)
    if [ "${STUB_COMMIT_SKILLS:-0}" = "1" ]; then
      echo "skills/from-worker.sh"
    else
      echo "prd/from-worker.md"
    fi
    ;;
  *"pr checks"*)
    echo '[{"name":"ci","state":"SUCCESS","bucket":"pass"}]'
    ;;
  *"pr view"*)
    echo '{"mergeable":"MERGEABLE","state":"OPEN","headRefName":"main"}'
    ;;
  *"pr merge"*)
    exit 0
    ;;
  *"repo view"*)
    echo '{"nameWithOwner":"stub/repo"}'
    ;;
  *)
    exit 0
    ;;
esac
EOF
chmod +x "$STUB_BIN/gh"
export PATH="$STUB_BIN:$PATH"
export ORCH_PROGRESS=0
export ORCH_TMUX=0

: >"$MERGE_LOG"
set +e
out_meta="$(STUB_COMMIT_META=1 GH_STUB_PR_NUM=301 \
  bash "$SCRIPTS/orch.sh" --repo "$TMP" --mode ready-pickup --issue 21 --agent "$STUB_BIN/stub-agent" 2>&1)"
rc_meta=$?
set -e
assert_eq "$rc_meta" "0" "allowlist worker orch exits 0"
assert_contains "$out_meta" "merged" "orch reports merged on allowlist"
assert_contains "$(cat "$MERGE_LOG")" "pr merge 301" "harness invoked gh pr merge on allow"
orch_worktree_remove "$TMP" "$(orch_worktree_path "$TMP" 21 issue-21)" 2>/dev/null || true

: >"$MERGE_LOG"
set +e
out_sk="$(STUB_COMMIT_SKILLS=1 GH_STUB_PR_NUM=302 \
  bash "$SCRIPTS/orch.sh" --repo "$TMP" --mode ready-pickup --issue 22 --agent "$STUB_BIN/stub-agent" 2>&1)"
rc_sk=$?
set -e
assert_eq "$rc_sk" "0" "skills worker orch still done (PR left open)"
assert_contains "$out_sk" "done: issue 22 (PR 302 checks green" "skills path does not merge"
if grep -q "pr merge" "$MERGE_LOG"; then
  FAILS=$((FAILS + 1))
  printf 'FAIL harness must not gh pr merge skills diff\n%s\n' "$(cat "$MERGE_LOG")" >&2
else
  PASSES=$((PASSES + 1))
fi
orch_worktree_remove "$TMP" "$(orch_worktree_path "$TMP" 22 issue-22)" 2>/dev/null || true

summary
