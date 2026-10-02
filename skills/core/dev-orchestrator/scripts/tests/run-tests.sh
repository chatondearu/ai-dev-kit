#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=helpers.sh
source "$ROOT/helpers.sh"
# Individual test files source helpers and call summary — appended by later tasks
printf 'No lib tests registered yet\n'
summary
