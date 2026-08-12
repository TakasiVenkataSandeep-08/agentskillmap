#!/usr/bin/env bash
# Checks that the tools a caller needs are installed, and explains how to use
# them. Naming a command is not running one.
set -euo pipefail

# curl is used by the publish script, not by this one.
if ! command -v curl >/dev/null 2>&1; then
  echo "curl is required; install it with your package manager" >&2
  exit 1
fi

if ! which wget >/dev/null 2>&1; then
  echo "Optional: wget -q https://example.invalid/manifest.json -O manifest.json" >&2
fi

# A networked CLI is process execution, not egress: nothing here can see inside
# `gh`, so this rule deliberately does not claim it.
gh release list --limit 5

echo "Run: ssh deploy@host 'systemctl restart app'"
