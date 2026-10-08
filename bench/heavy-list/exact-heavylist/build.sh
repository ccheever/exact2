#!/bin/bash
# Builds the exact2 app for a device from the checkout this directory is in: an unsigned archive from
# host/apple/build.mjs (EXACT_APP_DIR names this directory), unpacked to build/HeavyExact2.app, signed with the probe
# by ../probe/resign.sh and stamped BenchBuild=heavy@<commit> (+dirty when tracked files differ from it). The old
# bundle is deleted first and any failure aborts, so a series never measures a stale build. The simulator build is
# build.mjs --ios (README.md).
set -euo pipefail
H=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$H/../../.." && pwd)
"$H/../prepare.sh"
OUT=$H/build/HeavyExact2.app; rm -rf "$OUT"; mkdir -p "$H/build"
REV=$(git -C "$ROOT" rev-parse --short=9 HEAD)$([ -z "$(git -C "$ROOT" status --porcelain --untracked-files=no)" ] || echo +dirty)
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
(cd "$ROOT" && EXACT_APP_DIR=$H bun host/apple/build.mjs --device exact-heavylist-apple --archive "$T/app.ipa" --unsigned) \
  > "$H/build/build.log" 2>&1 || { echo "BUILD FAILED at $REV (see $H/build/build.log)"; exit 1; }
[ -s "$T/app.ipa" ] || { echo "BUILD FAILED at $REV: no archive"; exit 1; }
(cd "$T" && ditto -x -k app.ipa x)
mv "$T"/x/Payload/*.app "$OUT"
echo "$REV" > "$H/build/HeavyExact2.app.commit"
BENCH_BUILD=heavy@$REV "$H/../probe/resign.sh" "$OUT" dev.exact.heavybench.exact | tail -1
echo "built heavy@$REV: $OUT"
