#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Must NOT fire: a literal tail names a specific file inside the bundle. That is
# an import, not an evaluation, and two bundles in the labelled corpus do it.
source "$SCRIPT_DIR/lib.sh"

# Must NOT fire: eval on a literal is a quoting idiom and evaluates nothing the
# reader cannot already see.
eval echo hello

# Must NOT fire: stdin is data here, and the script is named on the command line.
cat payload.json | python3 render.py

# Must NOT fire: piping into a filter is not piping into an interpreter.
cat manifest.json | jq -r '.version'
printf '%s\n' "$SCRIPT_DIR" | grep -q skill
