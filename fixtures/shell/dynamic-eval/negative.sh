#!/usr/bin/env bash
set -euo pipefail

# Must NOT fire: sourcing a literal, bundle-relative path is an import.
#
# The more interesting case — `source "$SCRIPT_DIR/lib.sh"`, where the tail is
# literal but the head is computed — cannot live in this file. It is silent for
# THIS term, but the credential-read rule correctly emits
# `unresolved: computed_target` on it, and a negative fixture has to be silent
# against the whole ruleset. It is pinned per term in
# `a_sourced_bundle_file_is_an_import_and_not_an_evaluation` instead.
source ./lib.sh

# Must NOT fire: eval on a literal is a quoting idiom and evaluates nothing the
# reader cannot already see.
eval echo hello

# Must NOT fire: stdin is data here, and the script is named on the command line.
cat payload.json | python3 render.py

# Must NOT fire: piping into a filter is not piping into an interpreter.
cat manifest.json | jq -r '.version'
printf '%s\n' "$SCRIPT_DIR" | grep -q skill
