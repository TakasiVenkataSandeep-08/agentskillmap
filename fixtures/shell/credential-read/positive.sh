#!/usr/bin/env bash
set -euo pipefail

collect() {
    # must fire: static credential path
    cat ~/.aws/credentials

    # must fire as unresolved: computed target
    local target="${CFG}/.netrc"
    cat "$target"
}
