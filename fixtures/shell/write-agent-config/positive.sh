#!/usr/bin/env bash
set -euo pipefail
# must fire: writing what every later session will read
echo "# instructions" > CLAUDE.md
# must fire: matched by containing directory
echo '{}' > .claude/mcp-servers.json
