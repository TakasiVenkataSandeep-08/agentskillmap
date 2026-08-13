#!/usr/bin/env bash
set -euo pipefail
# must fire: absolute, therefore outside the bundle
echo '{}' > /tmp/skill-state.json
# must fire: creating a directory outside the bundle
mkdir -p /tmp/skill-state
