#!/usr/bin/env bash
# Runs a configured probe. The program is whatever the caller set.
set -euo pipefail

RUNNER="${STEP_RUNNER:-python3}"

# must fire: the command word is an expansion
$RUNNER --version

# must fire: quoted expansion, same thing
"${RUNNER}" check

capture() {
  # must fire: run whatever argv this function was handed
  "$@" 2>&1
}

capture uname -a
