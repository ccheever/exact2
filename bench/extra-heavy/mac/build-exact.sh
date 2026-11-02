#!/bin/bash
# build-exact.sh <label> — the exact2 app for macOS (host/apple/build.mjs --bundle: arm64, ad-hoc signed, no hardened
# runtime, so dyld honours the probe's DYLD_INSERT_LIBRARIES), at this checkout's working tree (or EXACT2=<another
# exact2 checkout>; A=<app dir> for another copy of the app), copied to dist/<label>.app with the commit and any
# uncommitted diff stat in dist/<label>.commit. A failed build leaves no bundle (the old one is removed first).
# push.sh stages it on the bench Mac as apps/exact2<label>.app (label "" stages apps/exact2.app).
set -o pipefail
LABEL=${1?label (may be empty)}
H=$(cd "$(dirname "$0")" && pwd); B=$(cd "$H/.." && pwd)
W=${EXACT2:-$(cd "$B/../.." && pwd)}; A=${A:-$B/exact-xheavy}; D=$H/dist; N=${LABEL:-main}
mkdir -p "$D"; rm -rf "$D/$N.app" "$D/$N.commit"; L=$D/build-$N.log
(cd "$W" && EXACT_APP_DIR=$A bun host/apple/build.mjs exact-xheavy-apple --bundle) > "$L" 2>&1 || { grep -E "error|host/apple:" "$L" | tail -8; echo "build failed: $N"; exit 1; }
APP=$(ls -d "$A"/target/clients/*/dev.exact.xheavy.exact2/macos/*.app 2>/dev/null | head -1)
[ -d "$APP" ] || { echo "no bundle"; tail -5 "$L"; exit 1; }
cp -R "$APP" "$D/$N.app"
{ git -C "$W" rev-parse --short HEAD; git -C "$W" diff --stat HEAD; } > "$D/$N.commit"
echo "built $N at $(head -1 "$D/$N.commit"): $D/$N.app"
