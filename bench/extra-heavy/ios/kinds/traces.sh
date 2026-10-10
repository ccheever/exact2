#!/bin/bash
# traces.sh <kind>… — per kind, one hold of BENCH_DEVICE: Time Profiler around fling 0 for exact2 then SwiftUI.
# Out: $BENCH_RESULTS/kinds-trace/<kind>-<app>/ (default <checkout>/target/bench/extra-heavy/results).
H=$(cd "$(dirname "$0")" && pwd)
T=${BENCH_RESULTS:-$H/../../../../target/bench/extra-heavy/results}/kinds-trace
: "${BENCH_DEVICE:?set BENCH_DEVICE to the device UDID}"
for K in "$@"; do
  tag=${K//,/+}
  [ -n "$BENCH_LOCK" ] && $BENCH_LOCK take kinds >/dev/null
  for app in ${APPS:-exact2 swiftui}; do
    echo "$(date +%T) trace $tag $app"; $H/trace.sh $app $K $T/$tag-$app
  done
  [ -n "$BENCH_LOCK" ] && $BENCH_LOCK give kinds >/dev/null; sleep 15
done
echo traces done
