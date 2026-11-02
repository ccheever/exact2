#!/usr/bin/env bash
# setup.sh [dir]: clones Nolan Lawson's agent framework benchmark at the commit these results
# used and applies harness.patch (Exact's images, Colima on ARM64, a subscription's access token).
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
DIR=${1:-$HOME/bench-afb/harness}
UPSTREAM=https://github.com/nolanlawson/agent-framework-benchmark
BASE=b95ada03930ca3670c8ff6377c3af815df0ac040
[ -e "$DIR" ] && { echo "$DIR exists; name another directory" >&2; exit 1; }
git clone -q "$UPSTREAM" "$DIR"
git -C "$DIR" checkout -q -b exact "$BASE"
git -C "$DIR" -c user.name=bench -c user.email=bench@localhost am -q --committer-date-is-author-date "$HERE/harness.patch"
echo "harness ready at $DIR"
