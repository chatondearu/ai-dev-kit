#!/usr/bin/env bash
# Export foam wikilinks to foam/graph/export.json; optional merge with .orch/graph/current.json
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
REPO="$(pwd)"
MERGE_ORCH=0

usage() {
  cat <<'EOF'
foam-graph-export.sh [--repo DIR] [--merge-orch]
  Writes foam/graph/export.json under the repo (gitignored export is ok to commit for CI snapshots).
EOF
}

while [ $# -gt 0 ]; do
  case "$1" in
    --repo) REPO="$2"; shift;;
    --merge-orch) MERGE_ORCH=1;;
    -h|--help) usage; exit 0;;
    *) echo "unknown: $1" >&2; exit 2;;
  esac
  shift
done

REPO="$(cd "$REPO" && pwd)"
FOAM="$REPO/foam"
OUT_DIR="$FOAM/graph"
OUT="$OUT_DIR/export.json"
mkdir -p "$OUT_DIR"

if ! command -v jq >/dev/null 2>&1; then
  echo "jq required" >&2
  exit 1
fi

nodes='[]'
edges='[]'

if [ -d "$FOAM" ]; then
  while IFS= read -r -d '' f; do
    rel="${f#$REPO/}"
    id="${rel%.md}"
    id="${id//\//--}"
    nodes="$(jq --arg id "$id" --arg path "$rel" '. + [{id:$id,kind:"foam",path:$path}]' <<<"$nodes")"
    while read -r link; do
      [ -n "$link" ] || continue
      tgt="${link//\//--}"
      edges="$(jq --arg from "$id" --arg to "$tgt" '. + [{from:$from,to:$to,type:"wikilink"}]' <<<"$edges")"
    done < <(grep -oE '\[\[[^]|]+\]\]' "$f" 2>/dev/null | sed -E 's/^\[\[([^]|]+)(\|[^\]]+)?\]\]$/\1/' || true)
  done < <(find "$FOAM" -name '*.md' -print0 2>/dev/null)
fi

orch='null'
if [ "$MERGE_ORCH" = 1 ] && [ -f "$REPO/.orch/graph/current.json" ]; then
  orch="$(cat "$REPO/.orch/graph/current.json")"
fi

jq -n \
  --argjson nodes "$nodes" \
  --argjson edges "$edges" \
  --arg repo "$REPO" \
  --arg updated "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  --argjson orch "$orch" \
  '{repo:$repo,updated_at:$updated,foam:{nodes:$nodes,edges:$edges},orch:$orch}' >"$OUT"

printf 'wrote %s (%s nodes, %s edges)\n' "$OUT" "$(jq '.foam.nodes | length' "$OUT")" "$(jq '.foam.edges | length' "$OUT")"
