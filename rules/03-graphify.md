# Graphify code graph (opt-in)

When `CDA_GRAPHIFY=1` and `<repo>/graphify-out/graph.json` exists:

- Read `graphify-out/GRAPH_REPORT.md` (or query the graph) **before** grepping
  or reading source files to orient.
- Do **not** install Graphify from the kit (`ADR-001`); the host / `aitools`
  owns that.
- When the env is unset or the graph file is missing, behaviour is unchanged.
