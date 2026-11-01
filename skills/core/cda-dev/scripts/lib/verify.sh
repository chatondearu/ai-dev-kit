#!/usr/bin/env bash
# shellcheck shell=bash
# Source-only library (no main).

_orch_extract_agents_verify() {
  local file="$1"
  [ -f "$file" ] || return 1
  local match
  # 0. fenced block under a "Verify" heading (`## … Verify …` or `**Verify …**`)
  #    wins over a trailing `verify:` line (ADR-006): the block is the real suite.
  match="$(awk '
    /^(#+ |\*\*).*Verify/ { seen = 1; next }
    seen && /^```/ { if (in_fence) exit; in_fence = 1; next }
    in_fence { print }
  ' "$file" | sed -e '/^[[:space:]]*$/d' || true)"
  if [ -n "$match" ]; then
    printf '%s\n' "$match"
    return 0
  fi
  # 1. line like "verify: <cmd>" (case-insensitive)
  match="$(grep -m 1 -Ei '^[[:space:]]*(-[[:space:]]*)?verify[[:space:]]*:' "$file" | head -n 1 || true)"
  if [ -n "$match" ]; then
    local cmd
    cmd="$(echo "$match" | sed -E 's/^[[:space:]]*(-[[:space:]]*)?verify[[:space:]]*:[[:space:]]*//I' | tr -d '`' | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//')"
    if [ -n "$cmd" ]; then
      printf '%s\n' "$cmd"
      return 0
    fi
  fi
  # 2. command inside fenced code block (``` ... ```)
  match="$(awk '/^```/ { in_fence = !in_fence; next } in_fence && /(nix develop|npm test|pnpm test)/ { gsub(/^[ \t]+|[ \t]+$/, ""); print; exit }' "$file" || true)"
  if [ -n "$match" ]; then
    printf '%s\n' "$match"
    return 0
  fi
  # 3. inline fenced `nix develop...`, `npm test`, or `pnpm test`
  match="$(grep -m 1 -oE '`(nix develop[^`]*|npm test|pnpm test)`' "$file" | head -n 1 | tr -d '`' || true)"
  if [ -n "$match" ]; then
    printf '%s\n' "$match"
    return 0
  fi
  return 1
}

orch_resolve_verify() {
  local dir="$1"
  if [ -n "${ORCH_VERIFY:-}" ]; then
    printf '%s\n' "$ORCH_VERIFY"
    return 0
  fi
  if [ -f "$dir/AGENTS.md" ]; then
    local agents_cmd
    if agents_cmd="$(_orch_extract_agents_verify "$dir/AGENTS.md")" && [ -n "$agents_cmd" ]; then
      printf '%s\n' "$agents_cmd"
      return 0
    fi
  fi
  if [ -f "$dir/flake.nix" ] || [ -f "$dir/.envrc" ]; then
    if [ -f "$dir/package.json" ]; then
      printf 'nix develop -c npm test\n'
      return 0
    fi
    if [ -f "$dir/Makefile" ]; then
      printf 'nix develop -c make test\n'
      return 0
    fi
  fi
  if [ -f "$dir/pnpm-lock.yaml" ]; then
    printf 'pnpm test\n'
    return 0
  fi
  if [ -f "$dir/package.json" ]; then
    printf 'npm test\n'
    return 0
  fi
  if [ -f "$dir/Makefile" ]; then
    printf 'make test\n'
    return 0
  fi
  if [ -f "$dir/Cargo.toml" ]; then
    printf 'cargo test\n'
    return 0
  fi
  # Fail closed (ADR-006): no command → caller blocks. `orch: allow-no-verify` in
  # the issue body (orch.sh exports ORCH_ALLOW_NO_VERIFY=1) keeps the old `true`.
  if [ "${ORCH_ALLOW_NO_VERIFY:-0}" = "1" ]; then
    printf 'orch_resolve_verify: no verify command; allow-no-verify set, using true\n' >&2
    printf 'true\n'
    return 0
  fi
  printf "orch_resolve_verify: no verify command found (add 'orch: allow-no-verify' to the issue body, or set ORCH_VERIFY)\n" >&2
  return 1
}

# Reason string shared by every caller that blocks on a missing verify command
# (exported: workers run under `bash -c`).
export ORCH_NO_VERIFY_REASON="verify: no command (add 'orch: allow-no-verify' to the issue body to skip)"

# Write <prompt> + the last 40 lines of a failed verify log to <wt>/.orch/worker-prompt-retry.md
# and print that path, so the retry agent sees why verify failed (ADR-006).
orch_verify_retry_prompt() {
  local prompt="$1" wt="$2" log="$3"
  local dest="$wt/.orch/worker-prompt-retry.md"
  {
    cat "$prompt"
    printf '\n\n## Previous verify output (failed)\n\n```\n'
    tail -n 40 "$log" 2>/dev/null
    printf '```\n'
  } >"$dest"
  printf '%s\n' "$dest"
}

# Allowlist for untrusted trees (pr-fix). Free-form AGENTS.md verify is refused.
_orch_verify_is_allowed() {
  case "$1" in
    true|npm\ test|pnpm\ test|make\ test|cargo\ test) return 0 ;;
    "nix develop -c npm test"|"nix develop -c pnpm test"|"nix develop -c make test"|"nix develop -c cargo test") return 0 ;;
    *) return 1 ;;
  esac
}

orch_run_verify() {
  local dir="$1"
  local cmd
  cmd="$(orch_resolve_verify "$dir")" || return 1
  if [ "${ORCH_VERIFY_TRUST:-trusted}" = "untrusted" ]; then
    if ! _orch_verify_is_allowed "$cmd"; then
      printf 'orch_run_verify: refusing non-allowlisted verify (untrusted): %s\n' "$cmd" >&2
      printf 'orch_run_verify: set ORCH_VERIFY to an allowlisted command if needed\n' >&2
      return 1
    fi
  fi
  # set -e: a multi-line Verify block must fail on its first failing command.
  (cd "$dir" && set -e && eval "$cmd")
}
