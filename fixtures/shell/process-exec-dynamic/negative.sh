#!/usr/bin/env bash
# Every command word here is literal, which is the ambient condition in shell
# and is deliberately not reported. A rule that fired on these would fire on
# every script in the corpus.
set -euo pipefail

echo "checking tools"
uname -a
mkdir -p ./out
printf '%s\n' "done" > ./out/status.txt
