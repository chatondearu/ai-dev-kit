#!/usr/bin/env bash
# pr-fix refuses fork PRs and PRs whose head is the default branch: the agent
# pushes `origin HEAD:<head_ref>`, which would land on the base repo's main.
set -euo pipefail
TROOT="$(cd "$(dirname "$0")" && pwd)"
SCRIPTS="$(cd "$TROOT/.." && pwd)"
# shellcheck source=helpers.sh
source "$TROOT/helpers.sh"

TMP="$(mktemp -d)"
STUB_BIN="$(mktemp -d)"
_ORIG_PATH="$PATH"
cleanup() {
  PATH="$_ORIG_PATH"
  git -C "$TMP/repo" worktree prune 2>/dev/null || true
  rm -rf "$TMP" "$STUB_BIN"
}
trap cleanup EXIT

R="$TMP/repo"
mkdir -p "$R"
git -C "$R" init -q
git -C "$R" config user.email t@t
git -C "$R" config user.name t
echo init >"$R/README"
git -C "$R" add README
git -C "$R" commit -qm init

# Stub agent leaves a marker: the gate must stop before any agent run.
cat >"$STUB_BIN/stub-agent" <<EOF
#!/usr/bin/env bash
cat >/dev/null
touch "$TMP/agent-ran"
EOF
chmod +x "$STUB_BIN/stub-agent"

cat >"$STUB_BIN/gh" <<'EOF'
#!/usr/bin/env bash
case "$*" in
  *"pr view"*headRefName*) echo "${GH_STUB_HEAD-feat/x}" ;;
  *"pr view"*isCrossRepository*) echo "${GH_STUB_CROSS:-false}" ;;
  *"repo view"*defaultBranchRef*) echo main ;;
  *) exit 0 ;;
esac
EOF
chmod +x "$STUB_BIN/gh"
export PATH="$STUB_BIN:$PATH"
export ORCH_AGENT="$STUB_BIN/stub-agent" ORCH_PROGRESS=0 ORCH_VERIFY=true ORCH_ALLOW_NO_CHECKS=1

run_pr_fix() {
  rm -f "$TMP/agent-ran"
  set +e
  out="$(bash "$SCRIPTS/orch.sh" --repo "$R" --mode pr-fix --pr "$1" --agent "$STUB_BIN/stub-agent" 2>&1)"
  rc=$?
  set -e
}

# The gate runs before checkout: a blocked run must not create the PR worktree.
assert_no_worktree() {
  if ls "$R"/.orch/worktrees/pr-"$2"* >/dev/null 2>&1; then
    FAILS=$((FAILS + 1)); printf 'FAIL %s: worktree created before the gate\n' "$1" >&2
  else
    PASSES=$((PASSES + 1))
  fi
}

assert_no_agent() {
  if [ -e "$TMP/agent-ran" ]; then
    FAILS=$((FAILS + 1)); printf 'FAIL %s: agent ran\n' "$1" >&2
  else
    PASSES=$((PASSES + 1))
  fi
}

echo "# cross-repository (fork) PR -> blocked, no agent run"
GH_STUB_CROSS=true run_pr_fix 41
assert_eq "$rc" "1" "fork PR exits nonzero"
assert_contains "$out" "blocked: PR 41 is cross-repository (fork)" "fork PR blocked message"
assert_no_agent "fork PR"
assert_no_worktree "fork PR" 41

echo "# same-repo PR whose head is the default branch -> blocked, no agent run"
GH_STUB_HEAD=main run_pr_fix 42
assert_eq "$rc" "1" "head=main exits nonzero"
assert_contains "$out" "blocked: PR 42 head branch is the default branch (main)" "default-branch head blocked message"
assert_no_agent "head=main PR"
assert_no_worktree "head=main PR" 42

echo "# PR metadata unreadable -> blocked (fail closed), no agent run"
GH_STUB_HEAD="" run_pr_fix 43
assert_eq "$rc" "1" "unreadable metadata exits nonzero"
assert_contains "$out" "blocked: PR 43 metadata unreadable" "unreadable metadata blocked message"
assert_no_agent "unreadable PR"
assert_no_worktree "unreadable PR" 43

echo "# same-repo feature branch -> gate passes, agent runs with the head ref"
run_pr_fix 44
if [ -e "$TMP/agent-ran" ]; then
  PASSES=$((PASSES + 1))
else
  FAILS=$((FAILS + 1)); printf 'FAIL same-repo PR must reach the agent\n%s\n' "$out" >&2
fi
WT="$(cd "$R" && pwd -P)/.orch/worktrees"
assert_contains "$(cat "$WT"/pr-44*/.orch/pr-fix-prompt.md)" "git push origin HEAD:feat/x" "prompt pushes to the PR head ref"

summary
