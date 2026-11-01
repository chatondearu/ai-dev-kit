#!/usr/bin/env bash
# ADR-006: fenced Verify block > verify: line; fail-closed when no verify command;
# independent review gate in ready-pickup (approved / changes_requested twice).
set -euo pipefail
TROOT="$(cd "$(dirname "$0")" && pwd)"
SCRIPTS="$(cd "$TROOT/.." && pwd)"
source "$TROOT/helpers.sh"
source "$TROOT/../lib/verify.sh"
source "$TROOT/../lib/worktree.sh"

TMP="$(mktemp -d)"
STUB_BIN="$(mktemp -d)"
_ORIG_PATH="$PATH"
cleanup() {
  PATH="$_ORIG_PATH"
  for d in "$TMP"/*/; do git -C "$d" worktree prune 2>/dev/null || true; done
  rm -rf "$TMP" "$STUB_BIN"
}
trap cleanup EXIT

echo "# 1. fenced Verify block wins over a trailing verify: line"
A="$TMP/agents"; mkdir -p "$A"
cat >"$A/AGENTS.md" <<'EOF'
# Agent guide

**Verify (run before every PR):**

```bash
bash tests/a.sh
bash tests/b.sh
```

verify: echo shadow-line
EOF
cmd="$(orch_resolve_verify "$A")"
assert_eq "$cmd" "$(printf 'bash tests/a.sh\nbash tests/b.sh')" "bold Verify block wins, keeps every line"

cat >"$A/AGENTS.md" <<'EOF'
## Verify
```bash
for t in tests/*.sh; do bash "$t"; done
```
verify: echo shadow-line
EOF
assert_eq "$(orch_resolve_verify "$A")" 'for t in tests/*.sh; do bash "$t"; done' "heading Verify block wins"

printf 'verify: echo only-line\n' >"$A/AGENTS.md"
assert_eq "$(orch_resolve_verify "$A")" "echo only-line" "verify: line still works without a block"

echo "# 1b. multi-line block fails on its first failing command (set -e)"
printf '## Verify\n```bash\nfalse\necho reached\n```\n' >"$A/AGENTS.md"
set +e
out_block="$(orch_run_verify "$A" 2>&1)"
rc_block=$?
set -e
assert_eq "$rc_block" "1" "block verify exits nonzero on first failure"
if [[ "$out_block" == *reached* ]]; then
  FAILS=$((FAILS + 1)); printf 'FAIL set -e must stop the block after false\n' >&2
else
  PASSES=$((PASSES + 1))
fi

echo "# 2. no verify command -> resolve fails; allow-no-verify -> true"
E="$TMP/empty"; mkdir -p "$E"
set +e
out_none="$(orch_resolve_verify "$E" 2>&1)"
rc_none=$?
set -e
assert_eq "$rc_none" "1" "resolve fails with no heuristic match"
assert_contains "$out_none" "orch: allow-no-verify" "failure names the escape hatch"
set +e
orch_run_verify "$E" >/dev/null 2>&1
rc_run_none=$?
set -e
assert_eq "$rc_run_none" "1" "run_verify is not green when resolve fails"
assert_eq "$(ORCH_ALLOW_NO_VERIFY=1 orch_resolve_verify "$E" 2>/dev/null)" "true" "allow-no-verify keeps true"
assert_eq "$(ORCH_VERIFY="make check" orch_resolve_verify "$E" 2>/dev/null)" "make check" "ORCH_VERIFY overrides resolution"

echo "# 3. ready-pickup review gate (stub gh + stub agent)"
# Stub agent: reads the prompt on stdin; verifier -> verifier-ok; reviewer -> verdict
# from STUB_REVIEW_VERDICTS (space-separated per round, last one repeats).
cat >"$STUB_BIN/stub-agent" <<'EOF'
#!/usr/bin/env bash
prompt="$(cat)"
mkdir -p .orch
case "$prompt" in
  *"Verifier (grounding node)"*) touch .orch/verifier-ok ;;
  *"Reviewer prompt"*)
    n=0; [ -f .orch/review-count ] && n="$(cat .orch/review-count)"; n=$((n + 1))
    echo "$n" >.orch/review-count
    # Done worktrees are removed at run end: keep what the tests inspect outside.
    mkdir -p "$STUB_KEEP"; cp .orch/review-count "$STUB_KEEP/review-count"
    printf "%s\n" "$prompt" >"$STUB_KEEP/reviewer-prompt-$n.md"
    set -- ${STUB_REVIEW_VERDICTS:-approved}
    v="${!n:-}"; [ -n "$v" ] || v="${@: -1}"
    echo "- [Important] foo.sh:1 — stub finding"
    echo "REVIEW: $v"
    ;;
esac
exit 0
EOF
chmod +x "$STUB_BIN/stub-agent"
export STUB_KEEP="$TMP/keep"

cat >"$STUB_BIN/gh" <<'EOF'
#!/usr/bin/env bash
case "$*" in
  *"issue view"*"--json body"*) printf '%s\n' "${GH_STUB_BODY:-Goal}" ;;
  *"issue view"*) echo "Test Issue" ;;
  *"pr list"*) echo '[{"number":55}]' ;;
  *"pr checks"*) echo '[{"name":"ci","state":"SUCCESS","bucket":"pass"}]' ;;
  *"pr view"*) echo '{"mergeable":"MERGEABLE","state":"OPEN","headRefName":"main"}' ;;
  *) exit 0 ;;
esac
EOF
chmod +x "$STUB_BIN/gh"
export PATH="$STUB_BIN:$PATH"
export ORCH_AGENT="$STUB_BIN/stub-agent" ORCH_PROGRESS=0 ORCH_FRONTIER=0

# Repo with a Verify block and a PLAN for issue 1 (full lane, no brief needed).
mk_repo() {
  local r="$1" with_verify="$2"
  mkdir -p "$r/plans"
  git -C "$r" init -q
  git -C "$r" config user.email t@t
  git -C "$r" config user.name t
  echo plan >"$r/plans/PLAN-1-test.md"
  if [ "$with_verify" = 1 ]; then
    printf '# Guide\n\n## Verify\n\n```bash\ntrue\n```\n' >"$r/AGENTS.md"
  fi
  git -C "$r" add -A
  git -C "$r" commit -qm init
}

R1="$TMP/r1"; mkdir -p "$R1"; mk_repo "$R1" 1
set +e
out_ok="$(bash "$SCRIPTS/orch.sh" --repo "$R1" --mode ready-pickup --issue 1 2>&1)"
rc_ok=$?
set -e
assert_eq "$rc_ok" "0" "approved review -> done"
assert_contains "$out_ok" "review: reviewer round 1 (PR #55)" "reviewer step shown"
assert_contains "$out_ok" "done: issue 1 (PR 55" "done after approved review"
WT1="$(orch_worktree_path "$R1" 1 issue-1)"
assert_file "$STUB_KEEP/reviewer-prompt-1.md" "reviewer prompt rendered in a fresh file"
assert_contains "$(cat "$STUB_KEEP/reviewer-prompt-1.md")" "Plan: \`plans/PLAN-1-test.md\`" "reviewer prompt has PLAN_PATH"
assert_contains "$(cat "$STUB_KEEP/reviewer-prompt-1.md")" "Worktree cwd: $WT1" "reviewer prompt has WORKTREE"
assert_eq "$(cat "$STUB_KEEP/review-count")" "1" "approved -> exactly one review"
if [ -e "$WT1/.orch/worker-prompt-review-fix.md" ]; then
  FAILS=$((FAILS + 1)); printf 'FAIL approved review must not trigger a fix iteration\n' >&2
else
  PASSES=$((PASSES + 1))
fi

echo "# 3b. changes_requested twice -> one fix iteration, then blocked"
R2="$TMP/r2"; mkdir -p "$R2"; mk_repo "$R2" 1
set +e
out_cr="$(STUB_REVIEW_VERDICTS="changes_requested" bash "$SCRIPTS/orch.sh" --repo "$R2" --mode ready-pickup --issue 1 2>&1)"
rc_cr=$?
set -e
assert_eq "$rc_cr" "1" "changes_requested twice -> exit nonzero"
assert_contains "$out_cr" "agent: review fix iteration" "one fix iteration ran"
assert_contains "$out_cr" "review: reviewer round 2" "re-review ran once"
assert_contains "$out_cr" "blocked: issue 1 review: changes_requested" "blocked with review reason"
WT2="$(orch_worktree_path "$R2" 1 issue-1)"
assert_eq "$(cat "$WT2/.orch/review-count")" "2" "exactly two reviews"
assert_contains "$(cat "$WT2/.orch/worker-prompt-review-fix.md")" "## Review findings (changes_requested)" "fix prompt has findings section"
assert_contains "$(cat "$WT2/.orch/worker-prompt-review-fix.md")" "[Important] foo.sh:1" "fix prompt carries reviewer findings"
if grep -q "^done: issue 1" <<<"$out_cr"; then
  FAILS=$((FAILS + 1)); printf 'FAIL done must not be reported after changes_requested twice\n' >&2
else
  PASSES=$((PASSES + 1))
fi

echo "# 3c. changes_requested then approved -> done"
R3="$TMP/r3"; mkdir -p "$R3"; mk_repo "$R3" 1
set +e
out_fix="$(STUB_REVIEW_VERDICTS="changes_requested approved" bash "$SCRIPTS/orch.sh" --repo "$R3" --mode ready-pickup --issue 1 2>&1)"
rc_fix=$?
set -e
assert_eq "$rc_fix" "0" "fix iteration then approved -> done"
assert_contains "$out_fix" "done: issue 1 (PR 55" "done after fix + approval"

echo "# 3d. no verify command -> blocked; orch: allow-no-verify in the body -> done"
R4="$TMP/r4"; mkdir -p "$R4"; mk_repo "$R4" 0
set +e
out_nov="$(bash "$SCRIPTS/orch.sh" --repo "$R4" --mode ready-pickup --issue 1 2>&1)"
rc_nov=$?
set -e
assert_eq "$rc_nov" "1" "no verify command -> exit nonzero"
assert_contains "$out_nov" "blocked: issue 1 verify: no command (add 'orch: allow-no-verify'" "blocked reason names the hatch"
if grep -q "^done: issue 1" <<<"$out_nov"; then
  FAILS=$((FAILS + 1)); printf 'FAIL done must not be reported without a verify command\n' >&2
else
  PASSES=$((PASSES + 1))
fi

R5="$TMP/r5"; mkdir -p "$R5"; mk_repo "$R5" 0
set +e
out_allow="$(GH_STUB_BODY=$'Goal\norch: allow-no-verify\n' bash "$SCRIPTS/orch.sh" --repo "$R5" --mode ready-pickup --issue 1 2>&1)"
rc_allow=$?
set -e
assert_eq "$rc_allow" "0" "allow-no-verify in body -> done"
assert_contains "$out_allow" "done: issue 1 (PR 55" "done with allow-no-verify"

summary
