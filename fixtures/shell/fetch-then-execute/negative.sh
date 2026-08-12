#!/usr/bin/env bash
# Fetches to a file, checks it, and tells the operator to run it. Nothing here
# hands bytes to an interpreter, and the two steps are what make that true.
set -euo pipefail

# Must NOT fire: stdin is data and the script is named on the command line.
cat payload.json | python3 render.py

# Must NOT fire: a filter is not an interpreter.
cat manifest.json | jq -r '.version'
