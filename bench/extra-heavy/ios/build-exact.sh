#!/bin/bash
# build-exact.sh <label> [bundle-suffix] — the exact2 app's signed device archive, at this checkout's working tree
# (or EXACT2=<another exact2 checkout>; A=<app dir> for another copy of the app), unpacked into
# ../device/exact2-<label>/app/Payload/<App>.app (named for the app) and re-signed with the probe embedded as
# dev.exact.xheavy.<bundle-suffix> (default exact2), stamped BenchBuild=xheavy@<commit> (the probe writes the stamp
# into every result). The old bundle is deleted first, so a failed build leaves nothing to measure; the commit and
# any uncommitted diff are written to ../device/exact2-<label>/commit. Signing: BENCH_SIGN_IDENTITY (a SHA-1) and
# BENCH_PROFILE (a .mobileprovision covering dev.exact.xheavy.*), what the probe's resign.sh reads; build.mjs
# --archive gets them as EXACT_IDENTITY / EXACT_PROFILE (set those to override). A bundle built on a full disk is
# a stale-build trap: it refuses under 20 GB free.
set -euo pipefail
LABEL=${1:?label}; AS=${2:-exact2}
H=$(cd "$(dirname "$0")" && pwd); B=$(cd "$H/.." && pwd)
W=${EXACT2:-$(cd "$B/../.." && pwd)}; A=${A:-$B/exact-xheavy}
P=${BENCH_PROBE:-$B/../heavy-list/probe}
export EXACT_IDENTITY=${EXACT_IDENTITY:-${BENCH_SIGN_IDENTITY:?set BENCH_SIGN_IDENTITY (security find-identity -v -p codesigning)}}
export EXACT_PROFILE=${EXACT_PROFILE:-${BENCH_PROFILE:?set BENCH_PROFILE to a provisioning profile covering dev.exact.xheavy.*}}
free_gb=$(df -g "$B" | tail -1 | awk '{print $4}'); [ "$free_gb" -ge 20 ] || { echo "refusing to build with ${free_gb} GB free (need 20)"; exit 3; }
D=$B/device/exact2-$LABEL; rm -rf "$D"; mkdir -p "$D"; L=$D/build.log
REV=$(git -C "$W" rev-parse --short=9 HEAD)
(cd "$W" && EXACT_APP_DIR=$A bun host/apple/build.mjs --device exact-xheavy-apple --archive "$D/app.ipa") > "$L" 2>&1 \
  || { echo "BUILD FAILED at $REV: $(grep -E 'error|host/apple:' "$L" | tail -3) (see $L)"; exit 1; }
[ -s "$D/app.ipa" ] || { echo "BUILD FAILED: no archive"; exit 1; }
{ echo "$REV"; git -C "$W" diff --stat HEAD; } > "$D/commit"
DIRTY=; [ -z "$(git -C "$W" status --porcelain --untracked-files=no)" ] || DIRTY=+dirty
(cd "$D" && mkdir app && cd app && ditto -x -k ../app.ipa .) && rm "$D/app.ipa"
APP=$(ls -d "$D"/app/Payload/*.app | head -1)
BENCH_BUILD=xheavy@$REV$DIRTY "$P/resign.sh" "$APP" dev.exact.xheavy.$AS | tail -1
EXE=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$APP/Info.plist")
echo "built $LABEL at $REV$DIRTY: $APP (UUID $(dwarfdump --uuid "$APP/$EXE" | head -1 | cut -d' ' -f2))"
