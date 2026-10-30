#!/usr/bin/env bash
# headless.sh — run one agent CLI against a repo with verify loop (no Kanban / worktree).
# Uses the same detect + orch_loop_worker as orch.sh.
#
#   bash headless.sh --prompt "fix the flaky test" [--repo DIR] [--max-iter N]
#   bash headless.sh --prompt-file ./task.md --dry-run
set -euo pipefail
_self="${BASH_SOURCE[0]:-$0}"
_dir="${_self%/*}"
[ "$_dir" = "$_self" ] && _dir="."
ROOT="$(cd "$_dir" && pwd)"
# shellcheck source=lib/agents.sh
source "$ROOT/lib/agents.sh"
source "$ROOT/lib/verify.sh"
source "$ROOT/lib/loop.sh"
source "$ROOT/lib/graph.sh"
source "$ROOT/lib/sandbox.sh"

REPO="$(pwd)"
PROMPT_TEXT=""
PROMPT_FILE=""
DRY=0
SANDBOX=0
MAX_I="${ORCH_MAX_ITER:-10}"

usage() {
  cat <<'EOF'
headless.sh — agent + verify loop (no issue/PR/worktree)

  --repo DIR            working tree (default: cwd)
  --prompt TEXT         task prompt (mutually exclusive with --prompt-file)
  --prompt-file FILE    read prompt from file
  --agent CMD           ORCH_AGENT override (e.g. "claude -p")
  --max-iter N          verify retries (default: ORCH_MAX_ITER or 10)
  --sandbox             optional bwrap (ORCH_SANDBOX=1)
  --dry-run             print agent + verify command; do not invoke
  -h|--help
EOF
}

while [ $# -gt 0 ]; do
  case "$1" in
    --repo) REPO="$2"; shift;;
    --prompt) PROMPT_TEXT="$2"; shift;;
    --prompt-file) PROMPT_FILE="$2"; shift;;
    --agent) ORCH_AGENT="$2"; export ORCH_AGENT; shift;;
    --max-iter) MAX_I="$2"; shift;;
    --sandbox) SANDBOX=1;;
    --dry-run) DRY=1;;
    -h|--help) usage; exit 0;;
    *) echo "unknown: $1" >&2; usage >&2; exit 2;;
  esac
  shift
done

if [ -n "$PROMPT_TEXT" ] && [ -n "$PROMPT_FILE" ]; then
  echo "use only one of --prompt / --prompt-file" >&2
  exit 2
fi
if [ -z "$PROMPT_TEXT" ] && [ -z "$PROMPT_FILE" ]; then
  echo "need --prompt or --prompt-file" >&2
  usage >&2
  exit 2
fi

REPO="$(cd "$REPO" && pwd)"
export ORCH_REPO="$REPO" ORCH_MAX_ITER="$MAX_I" ORCH_SANDBOX="$SANDBOX"

# Detect twice: once for printed kind, once for ORCH_AGENT_BIN in this shell
# (command substitution would drop the export).
kind="$(orch_detect_agent)" || exit 1
orch_detect_agent >/dev/null || exit 1
vcmd="$(orch_resolve_verify "$REPO")"

WORKDIR="$REPO/.orch/headless"
mkdir -p "$WORKDIR"
PROMPT_PATH="$WORKDIR/prompt.md"
{
  printf '# Headless task\n\n'
  printf 'Work only in this repository. Do not open PRs unless asked.\n'
  printf 'When done, leave the tree in a state that passes local verify.\n\n'
  printf '## Task\n\n'
  if [ -n "$PROMPT_FILE" ]; then
    cat "$PROMPT_FILE"
  else
    printf '%s\n' "$PROMPT_TEXT"
  fi
} >"$PROMPT_PATH"

printf 'agent: %s (%s)\n' "$kind" "${ORCH_AGENT_BIN:-}"
printf 'repo:  %s\n' "$REPO"
printf 'verify: %s\n' "$vcmd"
printf 'prompt: %s\n' "$PROMPT_PATH"
printf 'max-iter: %s\n' "$MAX_I"

if [ "$DRY" = 1 ]; then
  printf 'dry-run: skip agent invoke\n'
  exit 0
fi

if [ "$SANDBOX" = 1 ]; then
  export ORCH_WT="$REPO"
  export -f orch_loop_worker orch_graph_worker orch_agent_run orch_run_verify orch_resolve_verify
  export -f orch_graph_enabled orch_graph_file orch_graph_now orch_graph_init
  export -f orch_graph_upsert_node orch_graph_add_edge orch_graph_touch orch_graph_verifier_pass
  export -f _orch_extract_agents_verify orch_detect_agent orch_render_prompt
  export -f orch_sandbox_wrap
  orch_graph_init "headless-$(date -u +%Y%m%dT%H%M%SZ)"
  if orch_sandbox_wrap bash -c \
    "ORCH_WT=\"$REPO\" ORCH_REPO=\"$REPO\" ORCH_MAX_ITER=\"$MAX_I\" orch_loop_worker \"$REPO\" \"$PROMPT_PATH\""; then
    orch_graph_finish ok
  else
    orch_graph_finish blocked
    exit 1
  fi
else
  orch_graph_init "headless-$(date -u +%Y%m%dT%H%M%SZ)"
  if orch_loop_worker "$REPO" "$PROMPT_PATH"; then
    orch_graph_finish ok
  else
    orch_graph_finish blocked
    exit 1
  fi
fi
