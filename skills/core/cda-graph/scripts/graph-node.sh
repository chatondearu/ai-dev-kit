#!/usr/bin/env bash
# CLI wrapper over cda-dev lib/graph.sh for Cursor session fallback (no free-form JSON).
set -euo pipefail
DEV_LIB="$(cd "$(dirname "$0")/../../cda-dev/scripts/lib" && pwd)"
# shellcheck source=../../cda-dev/scripts/lib/graph.sh
source "$DEV_LIB/graph.sh"

usage() {
  cat <<'EOF'
graph-node.sh — write .orch/graph/current.json (schema: templates/orch-graph.schema.json)

  graph-node.sh init [--repo DIR] [--run-id ID]
  graph-node.sh node --id ID --kind KIND --phase PHASE --status STATUS [--message MSG] [--parent ID] [--repo DIR]
  graph-node.sh edge --from ID --to ID [--type TYPE] [--repo DIR]
  graph-node.sh finish [--outcome ok|blocked] [--repo DIR]

Env: ORCH_REPO (default .), ORCH_GRAPH=1
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

case "$cmd" in
  init)
    orch_graph_init "${RUN_ID:-cursor-fallback-$(date -u +%Y%m%dT%H%M%SZ)}"
    printf '%s\n' "$(orch_graph_file)"
    ;;
  node)
    [ -n "$NODE_ID" ] && [ -n "$KIND" ] && [ -n "$PHASE" ] && [ -n "$STATUS" ] || {
      echo "node requires --id --kind --phase --status" >&2
      exit 2
    }
    [ -f "$(orch_graph_file)" ] || orch_graph_init "cursor-fallback-$(date -u +%Y%m%dT%H%M%SZ)"
    orch_graph_upsert_node "$NODE_ID" "$KIND" "$PHASE" "$STATUS" "${MSG:-}" "${PARENT:-}"
    ;;
  edge)
    [ -n "$EDGE_FROM" ] && [ -n "$EDGE_TO" ] || {
      echo "edge requires --from --to" >&2
      exit 2
    }
    [ -f "$(orch_graph_file)" ] || { echo "missing graph; run init first" >&2; exit 1; }
    orch_graph_add_edge "$EDGE_FROM" "$EDGE_TO" "$EDGE_TYPE"
    ;;
  finish)
    orch_graph_finish "$OUTCOME"
    ;;
  *)
    usage
    exit 2
    ;;
esac
