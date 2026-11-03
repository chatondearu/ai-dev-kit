#!/usr/bin/env bash
# CLI wrapper over cda-dev lib/graph.sh for Cursor session fallback (no free-form JSON).
set -euo pipefail
DEV_LIB="$(cd "$(dirname "$0")/../../cda-dev/scripts/lib" && pwd)"
# shellcheck source=../../cda-dev/scripts/lib/graph.sh
source "$DEV_LIB/graph.sh"

usage() {
  cat <<'EOF'
graph-node.sh — write .orch/graph/live/<run_id>.json (schema: templates/orch-graph.schema.json)

  graph-node.sh init [--repo DIR] [--run-id ID]      new run; prints its file (basename = run id)
  graph-node.sh node --id ID --kind KIND --phase PHASE --status STATUS [--message MSG] [--parent ID] [--repo DIR] [--run-id ID]
  graph-node.sh edge --from ID --to ID [--type TYPE] [--repo DIR] [--run-id ID]
  graph-node.sh finish [--outcome ok|blocked] [--repo DIR] [--run-id ID]   moves the run to runs/

node/edge/finish target --run-id, else $ORCH_RUN_ID, else the newest live run created
from the current tmux session.

Env: ORCH_REPO (default .), ORCH_GRAPH=1, ORCH_RUN_ID
EOF
}

cmd="${1:-}"
[ -n "$cmd" ] || { usage; exit 2; }
shift || true

REPO="${ORCH_REPO:-.}"
RUN_ID=""
NODE_ID="" KIND="" PHASE="" STATUS="" MSG="" PARENT=""
EDGE_FROM="" EDGE_TO="" EDGE_TYPE="depends"
OUTCOME="ok"

while [ $# -gt 0 ]; do
  case "$1" in
    --repo) REPO="$2"; shift;;
    --run-id) RUN_ID="$2"; shift;;
    --id) NODE_ID="$2"; shift;;
    --kind) KIND="$2"; shift;;
    --phase) PHASE="$2"; shift;;
    --status) STATUS="$2"; shift;;
    --message) MSG="$2"; shift;;
    --parent) PARENT="$2"; shift;;
    --from) EDGE_FROM="$2"; shift;;
    --to) EDGE_TO="$2"; shift;;
    --type) EDGE_TYPE="$2"; shift;;
    --outcome) OUTCOME="$2"; shift;;
    -h|--help) usage; exit 0;;
    *) echo "unknown: $1" >&2; usage; exit 2;;
  esac
  shift
done

export ORCH_REPO="$(cd "$REPO" && pwd)"
export ORCH_GRAPH="${ORCH_GRAPH:-1}"

# Resolve the target run for node/edge/finish and require its live file.
use_run() {
  local s f
  RUN_ID="${RUN_ID:-${ORCH_RUN_ID:-}}"
  if [ -z "$RUN_ID" ] && s="$(orch_graph_session)" && [ -n "$s" ]; then
    while IFS= read -r f; do
      if [ "$(jq -r '.session // empty' "$f" 2>/dev/null)" = "$s" ]; then
        RUN_ID="$(basename "$f" .json)"
        break
      fi
    done < <(ls -1t "$ORCH_REPO/.orch/graph/live"/*.json 2>/dev/null)
  fi
  if [ -z "$RUN_ID" ]; then
    echo "graph-node: no run to write to: pass --run-id ID, set ORCH_RUN_ID, or run 'graph-node.sh init' from this tmux session" >&2
    exit 1
  fi
  export ORCH_RUN_ID="$RUN_ID"
  [ -f "$(orch_graph_file)" ] || {
    echo "graph-node: no live graph for run $RUN_ID (finished or never initialized): $(orch_graph_file)" >&2
    exit 1
  }
}

case "$cmd" in
  init)
    # Never reuse $ORCH_RUN_ID here: init always starts a new run ($$ keeps same-second ids apart).
    orch_graph_init "${RUN_ID:-cursor-fallback-$(date -u +%Y%m%dT%H%M%SZ)-$$}"
    printf '%s\n' "$(orch_graph_file)"
    ;;
  node)
    [ -n "$NODE_ID" ] && [ -n "$KIND" ] && [ -n "$PHASE" ] && [ -n "$STATUS" ] || {
      echo "node requires --id --kind --phase --status" >&2
      exit 2
    }
    use_run
    orch_graph_upsert_node "$NODE_ID" "$KIND" "$PHASE" "$STATUS" "${MSG:-}" "${PARENT:-}"
    ;;
  edge)
    [ -n "$EDGE_FROM" ] && [ -n "$EDGE_TO" ] || {
      echo "edge requires --from --to" >&2
      exit 2
    }
    use_run
    orch_graph_add_edge "$EDGE_FROM" "$EDGE_TO" "$EDGE_TYPE"
    ;;
  finish)
    use_run
    orch_graph_finish "$OUTCOME"
    ;;
  *)
    usage
    exit 2
    ;;
esac
