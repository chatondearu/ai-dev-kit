#!/usr/bin/env bash
set -euo pipefail
TROOT="$(cd "$(dirname "$0")" && pwd)"
source "$TROOT/helpers.sh"
source "$TROOT/../lib/verify.sh"
source "$TROOT/../lib/ci.sh"

TMP="$(mktemp -d)"
_ORIG_PATH="$PATH"
trap 'PATH="$_ORIG_PATH"; rm -rf "$TMP"' EXIT

# fake repo with package.json test script
printf '{"scripts":{"test":"echo ok"}}\n' >"$TMP/package.json"
cmd="$(orch_resolve_verify "$TMP")"
assert_contains "$cmd" "npm" "resolve npm test"

# stub gh — all checks pass
FAKE="$(mktemp -d)"
STUB_BASH="$(command -v bash)"
cat >"$FAKE/gh" <<EOF
#!$STUB_BASH
# emulate: gh pr checks 1 --json name,state,bucket
echo '[{"name":"ci","state":"SUCCESS","bucket":"pass"}]'
EOF
chmod +x "$FAKE/gh"
PATH="$FAKE:$_ORIG_PATH"
out="$(orch_pr_checks_watch 1 5)"
assert_eq "$out" "green" "ci green"

# empty checks → green (no CI configured)
cat >"$FAKE/gh" <<EOF
#!$STUB_BASH
echo '[]'
EOF
chmod +x "$FAKE/gh"
out_empty="$(orch_pr_checks_watch 99 5)"
assert_eq "$out_empty" "green" "empty checks green"

# gh fails with empty stdout → not green; exit nonzero after timeout
cat >"$FAKE/gh" <<EOF
#!$STUB_BASH
exit 1
EOF
chmod +x "$FAKE/gh"
PATH="$FAKE:$_ORIG_PATH"
set +e
out_gh_fail="$(orch_pr_checks_watch 1 0)"
rc_gh_fail=$?
set -e
if [ "$out_gh_fail" = "green" ]; then
  FAILS=$((FAILS + 1))
  printf 'FAIL failed gh must not be green\n  got: green\n' >&2
else
  PASSES=$((PASSES + 1))
fi
if [ "$rc_gh_fail" -eq 0 ]; then
  FAILS=$((FAILS + 1))
  printf 'FAIL failed gh watch must exit nonzero\n' >&2
else
  PASSES=$((PASSES + 1))
fi
assert_eq "$out_gh_fail" "pending" "failed gh ends pending"

summary
