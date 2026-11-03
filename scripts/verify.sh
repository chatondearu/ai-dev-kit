#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

for t in skills/core/cda-dev/scripts/tests/test_*.sh \
  skills/core/cda-dev/scripts/tests/smoke_orch_dry.sh \
  scripts/tests/test_doctor.sh \
  skills/core/cda-kanban/scripts/tests/test_closest_status.sh \
  skills/core/cda-kanban/scripts/tests/test_gh_board.sh \
  skills/quality/cda-kiss/hooks/test-install.sh \
  cdamux/hooks/test.sh \
  skills/core/cda-doc-hygiene/scripts/tests/test_doc_hygiene.sh \
  skills/core/cda-agents/scripts/tests/test_setup_agents.sh \
  skills/core/cda-prd/scripts/tests/test_new_prd_plan.sh \
  skills/core/cda-foam/scripts/tests/test_import_kanban.sh; do
  bash "$t"
done
bash skills/core/cda-doc-hygiene/scripts/doc-hygiene.sh --check
bash skills/quality/cda-kiss/hooks/test.sh
bash skills/design/cda-design-loop/scripts/tests/test_seed_design.sh
bash -n skills/core/cda-kanban/scripts/gh-board.sh
bash skills/core/cda-kanban/scripts/gh-board.sh reconcile-backlog --help >/dev/null
(cd cdamux && cargo test)
