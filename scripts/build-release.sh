#!/usr/bin/env bash
#
# Build the release binary reproducibly.
#
# T9's acceptance criterion is that two builds of the same tag from clean
# checkouts are byte-identical. That is not a property Cargo gives you for free —
# it is a property of the flags below, and the reason this is a script rather
# than a few lines inside .github/workflows/release.yml is that the release job
# and the job that *verifies* reproducibility must use exactly the same flags.
# Two copies of a flag list is one copy too many for a promise this specific.
#
# What breaks byte-identity, and what each flag does about it:
#
#   Absolute source paths. rustc records the path of every source file for panic
#   messages and debug info. Two clean checkouts live in different directories,
#   so those strings differ — and on a developer machine they contain a username.
#   --remap-path-prefix rewrites both the workspace and the Cargo registry to
#   fixed tokens. This is also the whole of docs/00-tasks.md's "absolute build
#   paths and usernames must not reach the binary": it cannot live in a committed
#   .cargo/config.toml, because the FROM side is whatever machine is building.
#
#   C compiler paths. Five tree-sitter grammars compile C through the `cc` crate,
#   and a C compiler bakes __FILE__ into assertions. -ffile-prefix-map is the
#   C-side equivalent; it is passed via CFLAGS, which `cc` forwards.
#
#   Parallel codegen. Splitting a crate into codegen units lets the optimizer
#   make different choices between runs. codegen-units = 1 is already set in
#   [profile.release] for exactly this reason, alongside lto and strip.
#
#   A build timestamp, on Windows only. This one was not a guess: the first two
#   builds of this repository from different directories differed in exactly 24
#   bytes, and every one of them was the PE header's TimeDateStamp repeated
#   through the COFF header and the debug directory. /Brepro tells the MSVC
#   linker to write a hash of the content there instead of the clock. ELF and
#   Mach-O have no equivalent field, so the flag is applied only where it exists.
#
# Usage:  scripts/build-release.sh [output-dir]
#
set -euo pipefail

WORKSPACE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUTPUT="${1:-$WORKSPACE/dist}"

# Where Cargo keeps downloaded sources. Dependency paths are absolute in a way
# the workspace's own are not: Cargo invokes rustc with workspace sources named
# relatively, so `crates/skillmap-cli/src/main.rs` is already machine-independent,
# while a registry crate is compiled by its full path. That asymmetry is why the
# first version of this script looked like it worked — the workspace really was
# clean, and ninety-one dependency paths carrying a username were not.
CARGO_HOME_DIR="${CARGO_HOME:-$HOME/.cargo}"

# Asked of rustc rather than inferred from `uname`, because the host triple is
# the thing that actually decides which linker runs.
HOST="$(rustc -vV | sed -n 's/^host: //p')"
BINARY="skillmap"
case "$HOST" in
  *windows-*) BINARY="skillmap.exe" ;;
esac

# The prefix has to be spelled the way rustc spells it. Under Git Bash and MSYS
# this script sees `/c/Users/...` while rustc emits `C:\Users\...`, and
# --remap-path-prefix is an exact prefix match, so the POSIX form silently
# matches nothing. Both forms are passed; on Unix cygpath is absent and the
# native form is the only one.
remaps=()
add_remap() {
  remaps+=("--remap-path-prefix=$1=$2")
  if command -v cygpath >/dev/null 2>&1; then
    remaps+=("--remap-path-prefix=$(cygpath -w "$1")=$2")
  fi
}
add_remap "$WORKSPACE" "/skillmap"
add_remap "$CARGO_HOME_DIR" "/cargo"

case "$HOST" in
  # The MSVC linker writes the wall clock into the PE header. /Brepro replaces it
  # with a hash of the content. ELF and Mach-O have no such field.
  *windows-msvc) remaps+=("-Clink-arg=/Brepro") ;;
esac

# CARGO_ENCODED_RUSTFLAGS rather than RUSTFLAGS: Cargo splits RUSTFLAGS on
# whitespace, so a build path containing a space would silently produce two
# broken flags instead of one correct one. The encoded form is separated by
# \x1f and cannot be mangled that way.
CARGO_ENCODED_RUSTFLAGS="$(printf '%s\x1f' "${remaps[@]}")"
CARGO_ENCODED_RUSTFLAGS="${CARGO_ENCODED_RUSTFLAGS%$'\x1f'}"
export CARGO_ENCODED_RUSTFLAGS

# The C half, for the five tree-sitter grammars that compile through `cc`.
# A C compiler bakes __FILE__ into assertions.
cflags=""
for remap in "${remaps[@]}"; do
  case "$remap" in
    --remap-path-prefix=*) cflags="$cflags -ffile-prefix-map=${remap#--remap-path-prefix=}" ;;
  esac
done
export CFLAGS="${cflags# }${CFLAGS:+ $CFLAGS}"

# Locked, not just committed: a release must build the dependency versions this
# repository tested, not whatever resolves today.
cargo build --release --locked -p skillmap-cli

BUILT="$WORKSPACE/target/release/$BINARY"
if [ ! -f "$BUILT" ]; then
  echo "build-release.sh: cargo reported success but $BUILT does not exist" >&2
  exit 1
fi

# Verify, do not assume. Every flag above is a claim about what did not end up in
# the binary, and the only way to know is to look. This check is the reason the
# registry leak was found at all — the flags looked right, the two builds were
# byte-identical, and the usernames were still there because both builds ran as
# the same user.
leaked=0
for secret in "$WORKSPACE" "$CARGO_HOME_DIR" "${HOME:-}"; do
  [ -n "$secret" ] || continue
  for form in "$secret" "$(command -v cygpath >/dev/null 2>&1 && cygpath -w "$secret" || echo "$secret")"; do
    if grep -a -q -F "$form" "$BUILT"; then
      echo "build-release.sh: the binary contains the build path \`$form\`." >&2
      leaked=1
    fi
  done
done
if [ "$leaked" -ne 0 ]; then
  echo "build-release.sh: refusing to publish a binary that names the machine that built it." >&2
  exit 1
fi

mkdir -p "$OUTPUT"
cp "$BUILT" "$OUTPUT/$BINARY"
echo "built $BINARY for $HOST"
( cd "$OUTPUT" && sha256sum "$BINARY" )
