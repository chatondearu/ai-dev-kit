#!/usr/bin/env bash
set -euo pipefail
TROOT="$(cd "$(dirname "$0")" && pwd)"
source "$TROOT/helpers.sh"

SCRIPTS="$(cd "$TROOT/.." && pwd)"
TMP="$(mktemp -d)"
FAKE="$(mktemp -d)"
STUB_BASH="$(command -v bash)"
trap 'rm -rf "$TMP" "$FAKE"' EXIT

cat >"$FAKE/claude" <<EOF
#!$STUB_BASH
exit 0
EOF
chmod +x "$FAKE/claude"

printf 'verify: true\n' >"$TMP/AGENTS.md"

# Force stub via --agent (no PATH surgery — NixOS keeps coreutils that way)
out="$("$STUB_BASH" "$SCRIPTS/headless.sh" --repo "$TMP" --prompt "noop" --agent "$FAKE/claude" --dry-run 2>&1)"
assert_contains "$out" "agent: custom" "dry-run detects custom agent"
assert_contains "$out" "verify: true" "dry-run shows verify"
assert_contains "$out" "dry-run: skip agent invoke" "dry-run skips invoke"
assert_file "$TMP/.orch/headless/prompt.md" "writes prompt under .orch/headless"

set +e
"$STUB_BASH" "$SCRIPTS/headless.sh" --repo "$TMP" --agent "$FAKE/claude" 2>/dev/null
rc=$?
set -e
assert_eq "$rc" "2" "missing prompt exits 2"

summary
