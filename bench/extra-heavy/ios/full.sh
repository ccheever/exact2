#!/bin/bash
export LOCK_HOLDER_PID=$$  # for a BENCH_LOCK that reclaims the lock when its holder dies
# full.sh — the full comparison on BENCH_DEVICE at one exact2 build (TAG names it; DEVN names the device in the
# series names, default "dev"):
#   1. the 19-kind series, 3 rounds, SwiftUI / Expo / exact2 / UIKit (series.sh; INSTALL puts the builds on first)
#   2. the per-kind breakdown for the kinds that lost before, 2 rounds (kinds/run.sh)
# Builds: ../device/exact2-$TAG/app/Payload/<App>.app (build-exact.sh) and the SwiftUI / Expo / UIKit apps in
# ../device/, all signed with the current probe.
H=$(cd "$(dirname "$0")" && pwd); X=$(cd "$H/.." && pwd); TAG=${TAG:-main}; DEVN=${DEVN:-dev}
: "${BENCH_DEVICE:?set BENCH_DEVICE to the device UDID}"
echo "$(date +%T) $DEVN xheavy"
SERIES=$TAG-19-$DEVN BENCH_KINDS= APPS="${XAPPS:-swiftui expo exact2 uikit}" \
  INSTALL="$(ls -d $X/device/exact2-$TAG/app/Payload/*.app | head -1) $X/device/XHeavy.app $X/device/XHeavyExpo.app $X/device/XHeavyUIKit.app" $H/series.sh
echo "$(date +%T) $DEVN kinds"
for r in 1 2; do
  SERIES=kinds-$TAG-$DEVN APPS="swiftui exact2 expo" $H/kinds/run.sh $r shader video map webview carousel svg
done
echo "$(date +%T) $DEVN full done"
