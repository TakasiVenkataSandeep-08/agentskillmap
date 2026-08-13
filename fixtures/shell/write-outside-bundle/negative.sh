#!/usr/bin/env bash
set -euo pipefail
# Must NOT fire: relative and descending.
mkdir -p out
echo '{}' > out/state.json
