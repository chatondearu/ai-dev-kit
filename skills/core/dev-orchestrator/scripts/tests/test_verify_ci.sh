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

# Tests for orch_pr_mergeable with stub gh
# 1. MERGEABLE and OPEN -> rc=0, prints MERGEABLE
cat >"$FAKE/gh" <<EOF
#!$STUB_BASH
echo '{"mergeable":"MERGEABLE","state":"OPEN"}'
EOF
set +e
m_out="$(orch_pr_mergeable 10)"
m_rc=$?
set -e
assert_eq "$m_rc" "0" "mergeable returns 0 for MERGEABLE"
assert_eq "$m_out" "MERGEABLE" "mergeable prints MERGEABLE"

# 2. UNKNOWN and OPEN -> rc=0, prints UNKNOWN
cat >"$FAKE/gh" <<EOF
#!$STUB_BASH
echo '{"mergeable":"UNKNOWN","state":"OPEN"}'
EOF
set +e
m_unk="$(orch_pr_mergeable 11)"
m_unk_rc=$?
set -e
assert_eq "$m_unk_rc" "0" "mergeable returns 0 for UNKNOWN"
assert_eq "$m_unk" "UNKNOWN" "mergeable prints UNKNOWN"

# 3. CONFLICTING -> rc=1, prints CONFLICTING
cat >"$FAKE/gh" <<EOF
#!$STUB_BASH
echo '{"mergeable":"CONFLICTING","state":"OPEN"}'
EOF
set +e
m_conf="$(orch_pr_mergeable 12)"
m_conf_rc=$?
set -e
assert_eq "$m_conf_rc" "1" "mergeable returns 1 for CONFLICTING"
assert_eq "$m_conf" "CONFLICTING" "mergeable prints CONFLICTING"

# 4. MERGEABLE but CLOSED -> rc=1
cat >"$FAKE/gh" <<EOF
#!$STUB_BASH
echo '{"mergeable":"MERGEABLE","state":"CLOSED"}'
EOF
set +e
m_closed="$(orch_pr_mergeable 13)"
m_closed_rc=$?
set -e
assert_eq "$m_closed_rc" "1" "mergeable returns 1 for closed PR"

# 5. gh command fails -> rc=1
cat >"$FAKE/gh" <<EOF
#!$STUB_BASH
exit 1
EOF
set +e
orch_pr_mergeable 14 >/dev/null 2>&1
m_fail_rc=$?
set -e
assert_eq "$m_fail_rc" "1" "mergeable returns 1 on gh command failure"

# Tests for AGENTS.md verification extraction in orch_resolve_verify
TMP_AGENTS="$(mktemp -d)"
echo "verify: nix develop -c npm test" >"$TMP_AGENTS/AGENTS.md"
cmd_agents="$(orch_resolve_verify "$TMP_AGENTS")"
assert_eq "$cmd_agents" "nix develop -c npm test" "resolve verify: from AGENTS.md"

cat >"$TMP_AGENTS/AGENTS.md" <<'EOF'
# Instructions
```bash
pnpm test
```
EOF
cmd_fenced="$(orch_resolve_verify "$TMP_AGENTS")"
assert_eq "$cmd_fenced" "pnpm test" "resolve fenced pnpm test from AGENTS.md"

cat >"$TMP_AGENTS/AGENTS.md" <<'EOF'
To test changes, run `npm test`.
EOF
cmd_inline="$(orch_resolve_verify "$TMP_AGENTS")"
assert_eq "$cmd_inline" "npm test" "resolve inline npm test from AGENTS.md"
rm -rf "$TMP_AGENTS"

summary
