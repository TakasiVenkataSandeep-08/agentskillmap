#!/usr/bin/env bash
#
# Materialize the npm platform packages from built binaries.
#
# esbuild's shape: one package per platform, each holding exactly one prebuilt
# binary and declaring `os` and `cpu`, plus a wrapper package that lists all of
# them as optionalDependencies. npm installs the one that matches and skips the
# rest, so nothing has to download anything at install time — which is the whole
# point, and is stated at length in npm/agentskillmap/bin/skillmap.
#
# These packages are generated rather than committed. Five package.json files
# differing in two fields each would be five files to keep in sync with the
# version in npm/agentskillmap/package.json, and the day they drift is the day npm
# installs a wrapper that cannot resolve its own binary.
#
# Usage:  scripts/npm-packages.sh <version> <artifacts-dir> <output-dir>
#
#   <artifacts-dir> holds one directory per target, each containing the binary:
#       artifacts/darwin-arm64/skillmap
#       artifacts/win32-x64/skillmap.exe
#
set -euo pipefail

if [ "$#" -ne 3 ]; then
  echo "usage: $0 <version> <artifacts-dir> <output-dir>" >&2
  exit 1
fi

VERSION="$1"
ARTIFACTS="$2"
OUTPUT="$3"
WORKSPACE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# npm target -> "os cpu". The keys are exactly what `process.platform` and
# `process.arch` return at runtime, which is what the wrapper joins to find its
# binary; keeping them identical means there is no mapping table to get wrong.
TARGETS=(
  "darwin-arm64:darwin:arm64"
  "darwin-x64:darwin:x64"
  "linux-arm64:linux:arm64"
  "linux-x64:linux:x64"
  "win32-x64:win32:x64"
)

mkdir -p "$OUTPUT"

# The wrapper, copied verbatim with its version replaced. Its
# optionalDependencies pin exact versions of the platform packages, so a
# published wrapper can never pair with a platform package built from different
# source.
cp -r "$WORKSPACE/npm/agentskillmap" "$OUTPUT/agentskillmap"
sed -i.bak -E "s/\"[0-9]+\.[0-9]+\.[0-9]+\"/\"$VERSION\"/g" "$OUTPUT/agentskillmap/package.json"
rm -f "$OUTPUT/agentskillmap/package.json.bak"
cp "$WORKSPACE/LICENSE" "$OUTPUT/agentskillmap/LICENSE"

for entry in "${TARGETS[@]}"; do
  target="${entry%%:*}"
  rest="${entry#*:}"
  os="${rest%%:*}"
  cpu="${rest#*:}"

  binary="skillmap"
  [ "$os" = "win32" ] && binary="skillmap.exe"

  source_binary="$ARTIFACTS/$target/$binary"
  if [ ! -f "$source_binary" ]; then
    # Not a warning. A missing platform means npm resolves no binary for it and
    # the wrapper exits 4 telling the user their platform is unsupported — a
    # release that quietly drops linux-x64 would look like a working release to
    # everybody except the majority of its users.
    echo "npm-packages.sh: $source_binary is missing; refusing to build a partial release" >&2
    exit 1
  fi

  package="$OUTPUT/@agentskillmap/$target"
  mkdir -p "$package/bin"
  cp "$source_binary" "$package/bin/$binary"
  chmod +x "$package/bin/$binary"
  cp "$WORKSPACE/LICENSE" "$package/LICENSE"

  cat > "$package/package.json" <<JSON
{
  "name": "@agentskillmap/$target",
  "version": "$VERSION",
  "description": "The skillmap binary for $target. Installed automatically by the \`skillmap\` package; not useful on its own.",
  "license": "Apache-2.0",
  "repository": {
    "type": "git",
    "url": "git+https://github.com/agentskillmap/agentskillmap.git"
  },
  "os": ["$os"],
  "cpu": ["$cpu"],
  "files": ["bin/$binary", "LICENSE"]
}
JSON

  echo "packaged @agentskillmap/$target"
done

echo
echo "wrapper and $(( ${#TARGETS[@]} )) platform packages in $OUTPUT"
