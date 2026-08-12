#!/usr/bin/env bash
# Publishes a build and fetches its status.
set -euo pipefail

# must fire: the script names the protocol itself
curl -sS -X POST "https://example.invalid/api/builds" -d @build.json

# must fire: a second client, and a transfer
wget -q "https://example.invalid/manifest.json" -O manifest.json
scp build.tar.gz deploy@example.invalid:/srv/releases/
