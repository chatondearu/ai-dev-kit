#!/usr/bin/env bash
set -euo pipefail
TROOT="$(cd "$(dirname "$0")" && pwd)"
SCRIPTS="$(cd "$TROOT/.." && pwd)"
# shellcheck source=helpers.sh
source "$TROOT/helpers.sh"
# shellcheck source=../lib/worktree.sh
source "$TROOT/../lib/worktree.sh"

TMP="$(mktemp -d)"
WT_PATH=""
WT_PATH2=""
_ORIG_PATH="$PATH"

cleanup() {
  PATH="$_ORIG_PATH"
  if [ -n "$WT_PATH" ] && [ -e "$WT_PATH" ]; then
    orch_worktree_remove "$TMP" "$WT_PATH" 2>/dev/null || true
  fi
  if [ -n "$WT_PATH2" ] && [ -e "$WT_PATH2" ]; then
    orch_worktree_remove "$TMP" "$WT_PATH2" 2>/dev/null || true
  fi
  rm -rf "$TMP"
}
trap cleanup EXIT

git -C "$TMP" init -q
git -C "$TMP" config user.email smoke@test
git -C "$TMP" config user.name smoke
echo init >"$TMP/README"
git -C "$TMP" add README
git -C "$TMP" commit -qm init
git -C "$TMP" branch -M main

# Setup stub agent that exits 0
STUB_BIN="$(mktemp -d)"
cat >"$STUB_BIN/stub-agent" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
chmod +x "$STUB_BIN/stub-agent"

# Setup stub npm so real verify ("npm test") succeeds
cat >"$STUB_BIN/npm" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
chmod +x "$STUB_BIN/npm"
export PATH="$STUB_BIN:$PATH"
export ORCH_AGENT="$STUB_BIN/stub-agent"

echo "# run-tests.sh"
bash "$TROOT/run-tests.sh"

echo "# orch dry-run (issues 1, 2)"
bash "$SCRIPTS/orch.sh" --repo "$TMP" --mode ready-pickup --issue 1 --issue 2 --dry-run

echo "# orch ready-pickup (issue 1, package.json missing -> verify true, stub agent exits 0)"
bash "$SCRIPTS/orch.sh" --repo "$TMP" --mode ready-pickup --issue 1 --agent "$STUB_BIN/stub-agent"

WT_PATH="$(orch_worktree_path "$TMP" 1 issue-1)"
assert_file "$WT_PATH" "worktree path after dispatch (issue 1)"
assert_file "$WT_PATH/.orch/worker-prompt.md" "rendered worker prompt exists under worktree (issue 1)"

orch_worktree_remove "$TMP" "$WT_PATH"
WT_PATH=""
if [ -e "$TMP/.orch/worktrees/1-issue-1" ]; then
  FAILS=$((FAILS + 1))
  printf 'FAIL worktree removed: %s/.orch/worktrees/1-issue-1\n' "$TMP" >&2
else
  PASSES=$((PASSES + 1))
fi

echo "# orch ready-pickup (issue 2, real verify with package.json test:true)"
printf '{"scripts":{"test":"true"}}\n' >"$TMP/package.json"
git -C "$TMP" add package.json
git -C "$TMP" commit -qm "add package.json"

bash "$SCRIPTS/orch.sh" --repo "$TMP" --mode ready-pickup --issue 2 --agent "$STUB_BIN/stub-agent"

WT_PATH2="$(orch_worktree_path "$TMP" 2 issue-2)"
assert_file "$WT_PATH2" "worktree path after dispatch (issue 2)"
assert_file "$WT_PATH2/.orch/worker-prompt.md" "rendered worker prompt exists under worktree (issue 2)"

orch_worktree_remove "$TMP" "$WT_PATH2"
WT_PATH2=""
if [ -e "$TMP/.orch/worktrees/2-issue-2" ]; then
  FAILS=$((FAILS + 1))
  printf 'FAIL worktree removed: %s/.orch/worktrees/2-issue-2\n' "$TMP" >&2
else
  PASSES=$((PASSES + 1))
fi

rm -rf "$STUB_BIN"

summary
