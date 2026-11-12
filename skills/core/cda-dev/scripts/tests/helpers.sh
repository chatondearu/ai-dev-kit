#!/usr/bin/env bash
# shellcheck shell=bash
set -euo pipefail

# Hermetic: drop harness-exported ORCH_* so unit tests see a clean process.
# Individual tests re-export what they need (ORCH_TMUX, ORCH_ALLOW_NO_CHECKS, …).
while IFS= read -r _orch_k; do
  unset "$_orch_k" 2>/dev/null || true
done < <(env | awk -F= '/^ORCH_/ { print $1 }')
unset _orch_k
# No parent-CLI inheritance in tests (ADR-010): no env markers, ps walk disabled.
unset CLAUDECODE CODEX_THREAD_ID CODEX_SESSION_ID CURSOR_INVOKED_AS
export ORCH_PS=false
# Headless by default in unit tests (production default is ORCH_TMUX=1).
export ORCH_TMUX=0

FAILS=0
PASSES=0

assert_eq() {
  local got="$1" want="$2" msg="${3:-}"
  if [ "$got" = "$want" ]; then
    PASSES=$((PASSES + 1))
  else
    FAILS=$((FAILS + 1))
    printf 'FAIL %s\n  got:  %s\n  want: %s\n' "$msg" "$got" "$want" >&2
  fi
}

assert_contains() {
  local hay="$1" needle="$2" msg="${3:-}"
  if [[ "$hay" == *"$needle"* ]]; then
    PASSES=$((PASSES + 1))
  else
    FAILS=$((FAILS + 1))
    printf 'FAIL %s\n  missing: %s\n  in: %s\n' "$msg" "$needle" "$hay" >&2
  fi
}

assert_file() {
  local path="$1" msg="${2:-file exists}"
  if [ -e "$path" ]; then
    PASSES=$((PASSES + 1))
  else
    FAILS=$((FAILS + 1))
    printf 'FAIL %s: %s\n' "$msg" "$path" >&2
  fi
}

summary() {
  printf 'passes=%s fails=%s\n' "$PASSES" "$FAILS"
  [ "$FAILS" -eq 0 ]
}

# Install a stub `cdamux` under $1/bin that writes $CDAMUX_HOME like the real CLI.
# Prepends that bin to PATH. Used by graph/progress hard-cut tests (#252).
orch_test_stub_cdamux() {
  local root="$1"
  mkdir -p "$root/bin"
  cat >"$root/bin/cdamux" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
case "${1:-}" in
  state)
    shift
    cmd="${1:-}"; shift || true
    kind="${1:-}"; id="${2:-}"; json="${3:-}"
    dir="${CDAMUX_HOME:?}/$kind"
    case "$cmd" in
      set)
        mkdir -p "$dir"
        if [ "$kind" = runs ] && [ "${CDAMUX_STUB_SLEEP_RUNS:-}" = invert ]; then
          n="$(printf '%s' "$json" | jq '.nodes | length' 2>/dev/null || echo 0)"
          sleep "$(awk -v n="$n" 'BEGIN { s = 0.20 - n * 0.02; if (s < 0.02) s = 0.02; printf "%.3f", s }')"
        fi
        printf '%s\n' "$json" >"$dir/${id}.json"
        ;;
      append)
        mkdir -p "$dir"
        f="$dir/${id}.json"
        if [ -f "$f" ] && command -v jq >/dev/null 2>&1; then
          jq -c --argjson p "$json" '. * $p' "$f" >"$f.tmp" && mv "$f.tmp" "$f"
        else
          printf '%s\n' "$json" >"$f"
        fi
        ;;
      get)
        f="$dir/${id}.json"
        [ -f "$f" ] || { echo "not found" >&2; exit 1; }
        cat "$f"
        ;;
      *) echo "stub: unknown state $cmd" >&2; exit 2;;
    esac
    ;;
  msg)
    shift
    [ "${1:-}" = send ] || { echo "stub: only msg send" >&2; exit 2; }
    shift
    to="" from="" kind="" ref="" text=""
    while [ $# -gt 0 ]; do
      case "$1" in
        --to) to="${2:-}"; shift 2;;
        --from) from="${2:-}"; shift 2;;
        --kind) kind="${2:-}"; shift 2;;
        --ref) ref="${2:-}"; shift 2;;
        --) shift; break;;
        -*) echo "stub: unknown msg flag $1" >&2; exit 2;;
        *) text="$1"; shift; break;;
      esac
    done
    [ -n "$to" ] && [ -n "$kind" ] && [ -n "$text" ] || { echo "stub: msg send needs --to --kind text" >&2; exit 2; }
    [ -n "$from" ] || from="${CDAMUX_AGENT_ID:-a-stub}"
    mkdir -p "${CDAMUX_HOME:?}/inbox"
    if command -v jq >/dev/null 2>&1; then
      jq -cn \
        --arg id "m-stub$(date +%s)" \
        --arg from "$from" --arg to "$to" --arg kind "$kind" \
        --arg text "$text" --arg ref "$ref" \
        '{id:$id,ts:1,from:$from,to:$to,kind:$kind,text:$text}
         + (if $ref=="" then {} else {ref:$ref} end)' \
        >>"$CDAMUX_HOME/inbox/${to}.jsonl"
    else
      printf '{"from":"%s","to":"%s","kind":"%s","text":"%s"}\n' \
        "$from" "$to" "$kind" "$text" >>"$CDAMUX_HOME/inbox/${to}.jsonl"
    fi
    echo m-stub1
    ;;
  *) echo "stub: only state|msg" >&2; exit 2;;
esac
EOF
  chmod +x "$root/bin/cdamux"
  export PATH="$root/bin:$PATH"
}
